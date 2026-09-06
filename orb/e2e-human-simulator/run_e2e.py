#!/usr/bin/env python3
"""Config-driven visible E2E run for the independent human simulator.

This orchestrator owns only harness processes. It starts explicitly configured
services, launches the app in a visible simulator, replays WAV fixtures through
the app's real WebSocket, captures checkpoint screenshots, and runs evaluation
after the journey has finished.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
from collections.abc import Mapping
from datetime import datetime, timezone
from pathlib import Path
from statistics import median
from typing import Any
from urllib.request import urlopen

from simulator.runner import SimulatorRunner, load_config
from transport.ws_proxy import APP_HEALTH_ALLOWED_TEXT, APP_HEALTH_ERROR_PATTERNS


class E2EError(RuntimeError):
    pass


COULD_NOT_RUN_CODES = (
    "APP_IDENTITY_UNAVAILABLE",
    "APP_IDENTITY_CHANGED",
    "APP_UNHEALTHY",
    "DUPLICATE_CHECKPOINT",
    "RELAY_TURN_NOT_ACCEPTED",
    "UI_TRANSITION_NOT_CONFIRMED",
)


def _failure_status(error: str) -> str:
    return "could-not-run" if any(code in error for code in COULD_NOT_RUN_CODES) else "ran-and-failed"


def _assert_app_healthy(ocr_payload: Mapping[str, Any]) -> None:
    text = " ".join(str(item) for item in ocr_payload.get("text", [])).lower()
    for allowed in APP_HEALTH_ALLOWED_TEXT:
        text = text.replace(allowed, "")
    errors = [pattern for pattern in APP_HEALTH_ERROR_PATTERNS if pattern in text]
    if errors:
        raise E2EError(f"APP_UNHEALTHY: launch OCR contains app/RN error text: {errors}")


def _trace_records(trace_path: Path) -> list[dict[str, Any]]:
    if not trace_path.exists():
        return []
    records: list[dict[str, Any]] = []
    for line in trace_path.read_text(encoding="utf-8", errors="replace").splitlines():
        try:
            record = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(record, dict):
            records.append(record)
    return records


def _timestamp_ms(value: Any) -> float | None:
    if isinstance(value, (int, float)) and not isinstance(value, bool):
        return float(value) * 1000.0
    if not isinstance(value, str):
        return None
    try:
        return datetime.fromisoformat(value.replace("Z", "+00:00")).timestamp() * 1000.0
    except ValueError:
        return None


def _turn_evidence(trace_path: Path) -> dict[str, Any]:
    records = _trace_records(trace_path)
    by_turn: dict[str, dict[str, Any]] = {}
    event_field = {
        "inject.end_of_turn": "inject",
        "stt.final": "transcript",
        "assistant.response": "assistant_response",
        "tts.started": "tts_started",
        "tts.first_audio": "tts_first_audio",
        "tts.complete": "tts_complete",
    }
    for record in records:
        field = event_field.get(str(record.get("event", "")))
        data = record.get("data")
        if field is None or not isinstance(data, dict):
            continue
        turn_id = data.get("turn_id")
        if not isinstance(turn_id, str) or not turn_id:
            continue
        by_turn.setdefault(turn_id, {"turn_id": turn_id})[field] = record
    turns: list[dict[str, Any]] = []
    for turn_id in sorted(by_turn):
        evidence = by_turn[turn_id]
        inject_ms = _timestamp_ms(evidence.get("inject", {}).get("ts"))
        first_audio_ms = _timestamp_ms(evidence.get("tts_first_audio", {}).get("ts"))
        latency_ms = None
        if inject_ms is not None and first_audio_ms is not None:
            latency_ms = round(max(0.0, first_audio_ms - inject_ms), 3)
        complete = all(field in evidence for field in ("inject", "transcript", "assistant_response", "tts_first_audio"))
        lifecycle_ms = {
            field: _timestamp_ms(evidence.get(field, {}).get("ts"))
            for field in ("tts_started", "tts_first_audio", "tts_complete")
        }
        lifecycle_valid = (
            all(value is not None for value in lifecycle_ms.values())
            and lifecycle_ms["tts_started"] < lifecycle_ms["tts_first_audio"] < lifecycle_ms["tts_complete"]
        )
        turns.append(
            {
                "turn_id": turn_id,
                "session_id": evidence.get("inject", {}).get("session_id"),
                "complete": complete,
                "inject_to_first_audio_ms": latency_ms,
                "tts_lifecycle_valid": lifecycle_valid,
                "tts_lifecycle_ms": lifecycle_ms,
                "events": {
                    field: evidence.get(field, {}).get("seq")
                    for field in (
                        "inject",
                        "transcript",
                        "assistant_response",
                        "tts_started",
                        "tts_first_audio",
                        "tts_complete",
                    )
                },
            }
        )
    latencies = [float(turn["inject_to_first_audio_ms"]) for turn in turns if turn["inject_to_first_audio_ms"] is not None]
    return {
        "turns": turns,
        "turns_complete": sum(bool(turn["complete"]) for turn in turns),
        "tts_lifecycles_valid": sum(bool(turn["tts_lifecycle_valid"]) for turn in turns),
        "p50_inject_to_first_audio_ms": None if not latencies else round(float(median(latencies)), 3),
    }


def _load(path: Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise E2EError(f"config must be an object: {path}")
    return payload


def _resolve(base: Path, value: str) -> Path:
    path = Path(value)
    return path if path.is_absolute() else (base / path).resolve()


def _wait_control(url: str, timeout_s: float = 20) -> None:
    from websocket import create_connection

    deadline = time.monotonic() + timeout_s
    last_error = ""
    while time.monotonic() < deadline:
        try:
            ws = create_connection(url, timeout=2)
            ws.close()
            return
        except Exception as exc:  # startup timing is environment-dependent
            last_error = str(exc)
            time.sleep(0.2)
    raise E2EError(f"proxy did not become ready: {last_error}")


def _control(url: str, payload: Mapping[str, Any]) -> dict[str, Any]:
    from websocket import create_connection

    ws = create_connection(url, timeout=15)
    try:
        ws.recv()
        ws.send(json.dumps(dict(payload)))
        response = json.loads(ws.recv())
        if response.get("type") == "error":
            raise E2EError(str(response.get("error")))
        return response
    finally:
        ws.close()


def _status(url: str) -> dict[str, Any]:
    return _control(url, {"type": "status"})


def _ensure_app(url: str, *, bundle_id: str, simulator_udid: str, timeout_s: float = 20) -> None:
    if _status(url).get("connected"):
        return
    _control(url, {"type": "relaunch"})
    deadline = time.monotonic() + timeout_s
    while time.monotonic() < deadline:
        if _status(url).get("connected"):
            return
        time.sleep(0.25)
    raise E2EError(f"app did not reconnect after relaunch: {bundle_id} on {simulator_udid}")


def _fresh_launch(url: str, *, bundle_id: str, simulator_udid: str, timeout_s: float = 30) -> None:
    generation = int(_status(url).get("connection_generation", 0))
    _control(url, {"type": "fresh_launch"})
    deadline = time.monotonic() + timeout_s
    while time.monotonic() < deadline:
        status = _status(url)
        if status.get("connected") and int(status.get("connection_generation", 0)) > generation:
            return
        time.sleep(0.25)
    raise E2EError(f"app did not connect after fresh launch: {bundle_id} on {simulator_udid}")


def _last_trace_seq(trace_path: Path) -> int:
    if not trace_path.exists():
        return 0
    last = 0
    for line in trace_path.read_text(encoding="utf-8", errors="replace").splitlines():
        try:
            last = max(last, int(json.loads(line).get("seq", 0)))
        except (json.JSONDecodeError, TypeError, ValueError):
            continue
    return last


def _wait_trace_event(trace_path: Path, events: set[str], *, after_seq: int, timeout_s: float) -> bool:
    deadline = time.monotonic() + timeout_s
    while time.monotonic() < deadline:
        if trace_path.exists():
            for line in trace_path.read_text(encoding="utf-8", errors="replace").splitlines():
                try:
                    record = json.loads(line)
                except json.JSONDecodeError:
                    continue
                if int(record.get("seq", 0)) > after_seq and str(record.get("event", "")) in events:
                    return True
        time.sleep(0.1)
    return False


def _assert_spoken_recovery(trace_path: Path, *, after_seq: int) -> None:
    observed = {
        str(record.get("event", ""))
        for record in _trace_records(trace_path)
        if int(record.get("seq", 0)) > after_seq
    }
    required = {"assistant.response", "tts.first_audio"}
    missing = sorted(required - observed)
    if missing:
        raise E2EError(
            "TURN_INCOMPLETE: injected fault produced no spoken recovery; "
            f"missing={','.join(missing)}"
        )


def _resolve_fixture(base: Path, fixture: str, roots: list[str]) -> Path:
    candidate = Path(fixture)
    if candidate.is_absolute() and candidate.is_file():
        return candidate
    for root in roots:
        path = _resolve(base, root) / fixture
        if path.is_file():
            return path
    direct = _resolve(base, fixture)
    if direct.is_file():
        return direct
    raise E2EError(f"fixture not found: {fixture} (searched {roots})")


def _run_replay(
    *,
    base: Path,
    env: dict[str, str],
    run_root: Path,
    control_url: str,
    fixture: Path,
    journey_id: str,
    step_index: int,
    session_id: str,
    speed: float,
    post_turn_wait: float,
    observation: str,
) -> dict[str, Any]:
    prefix = f"{journey_id}-{step_index:02d}"
    replay_args = [
        sys.executable,
        "-m",
        "transport.live_replay",
        str(fixture),
        "--control-url",
        control_url,
        "--session-id",
        session_id,
        "--speed",
        str(speed),
        "--snapshot-prefix",
        prefix,
        "--snapshot-observation",
        observation,
        "--post-turn-wait",
        str(post_turn_wait),
    ]
    log_path = run_root / f"replay-{prefix}.log"
    try:
        # No timeout here would let a stuck replay (dead socket recv, backend
        # deadlock) block the whole orchestrator forever, before it ever reaches
        # the caller's cleanup of owned processes (backend/proxy).
        completed = subprocess.run(
            replay_args, cwd=str(base), env=env, text=True, capture_output=True, check=False, timeout=120
        )
    except subprocess.TimeoutExpired as exc:
        stdout = exc.stdout.decode("utf-8", "replace") if isinstance(exc.stdout, bytes) else (exc.stdout or "")
        stderr = exc.stderr.decode("utf-8", "replace") if isinstance(exc.stderr, bytes) else (exc.stderr or "")
        log_path.write_text(stdout + stderr, encoding="utf-8")
        raise E2EError(f"replay timed out after 120s for {fixture}") from exc
    log_path.write_text(completed.stdout + completed.stderr, encoding="utf-8")
    if completed.returncode != 0:
        raise E2EError(
            f"replay failed for {fixture}: {completed.stdout[-1000:]} {completed.stderr[-1000:]}"
        )
    lines = [line for line in completed.stdout.splitlines() if line.strip()]
    try:
        replay_result = json.loads(lines[-1])
    except (IndexError, json.JSONDecodeError) as exc:
        raise E2EError(f"replay produced no structured acceptance result for {fixture}") from exc
    if not isinstance(replay_result, dict) or replay_result.get("status") != "accepted":
        raise E2EError(f"replay did not prove relay acceptance for {fixture}: {replay_result}")
    return replay_result


def _journey_plan(config: Mapping[str, Any], base: Path) -> list[dict[str, Any]] | None:
    journeys_file = config.get("journeys_file")
    if not journeys_file:
        return None
    payload = _load(_resolve(base, str(journeys_file)))
    journeys = payload.get("journeys")
    if not isinstance(journeys, list):
        raise E2EError(f"journeys_file must contain a journeys array: {journeys_file}")
    if not all(isinstance(journey, dict) for journey in journeys):
        raise E2EError(f"every journey must be an object: {journeys_file}")
    return journeys


def _run_journey_plan(
    journeys: list[dict[str, Any]],
    *,
    config: Mapping[str, Any],
    base: Path,
    env: dict[str, str],
    run_root: Path,
    trace_path: Path,
    control_url: str,
    bundle_id: str,
    simulator_udid: str,
) -> list[dict[str, Any]]:
    fixture_roots = [str(value) for value in config.get("fixture_roots", ["replay/fixtures", "runs/live-20260807"])]
    default_speed = float(config.get("journey_speed", 1.0))
    default_post_wait = float(config.get("post_turn_wait", 7.0))
    launch_wait = float(config.get("launch_wait", 3.0))
    barge_wait = float(config.get("barge_wait_for_tts_seconds", 15.0))
    observations = config.get("journey_observations", {})
    if not isinstance(observations, Mapping):
        observations = {}
    results: list[dict[str, Any]] = []

    for journey in journeys:
        journey_id = str(journey.get("id", "unnamed"))
        description = str(journey.get("description", ""))
        observation = str(observations.get(journey_id, f"Review screenshots and trace for: {description}"))
        result: dict[str, Any] = {
            "journey": journey_id,
            "ran": False,
            "status": "could-not-run",
            "outcome": "could-not-run",
            "error": None,
        }
        _control(
            control_url,
            {"type": "journey_marker", "journey_id": journey_id, "phase": "start", "description": description},
        )
        last_turn_seq = _last_trace_seq(trace_path)
        try:
            steps = journey.get("steps")
            if not isinstance(steps, list) or not steps:
                raise E2EError(f"journey has no executable steps: {journey_id}")
            for step_index, step in enumerate(steps):
                if not isinstance(step, Mapping):
                    raise E2EError(f"invalid step {step_index} in {journey_id}")
                if "expect" in step:
                    expectation = step["expect"]
                    _control(
                        control_url,
                        {
                            "type": "journey_marker",
                            "journey_id": journey_id,
                            "phase": "expectation",
                            "expectation": expectation,
                        },
                    )
                    _control(
                        control_url,
                        {
                            "type": "snapshot",
                            "label": f"{journey_id}-{step_index:02d}-expectation",
                            "observation": observation,
                        },
                    )
                    continue

                action = str(step.get("action", ""))
                if action == "fresh_launch":
                    _fresh_launch(control_url, bundle_id=bundle_id, simulator_udid=simulator_udid)
                    time.sleep(max(0.0, launch_wait))
                    _control(
                        control_url,
                        {
                            "type": "snapshot",
                            "label": f"{journey_id}-{step_index:02d}-launch",
                            "observation": observation,
                        },
                    )
                    result["ran"] = True
                elif action == "speak_fixture":
                    fixture = _resolve_fixture(base, str(step.get("fixture", "")), fixture_roots)
                    next_is_barge = (
                        step_index + 1 < len(steps)
                        and isinstance(steps[step_index + 1], Mapping)
                        and steps[step_index + 1].get("action") == "barge_in"
                    )
                    last_turn_seq = _last_trace_seq(trace_path)
                    _run_replay(
                        base=base,
                        env=env,
                        run_root=run_root,
                        control_url=control_url,
                        fixture=fixture,
                        journey_id=journey_id,
                        step_index=step_index,
                        session_id=f"e2e-{journey_id}",
                        speed=float(step.get("speed", default_speed)),
                        post_turn_wait=0.0 if next_is_barge else float(step.get("post_turn_wait", default_post_wait)),
                        observation=observation,
                    )
                    result["ran"] = True
                elif action == "inject_fault":
                    fault = str(step.get("fault", "provider_failure"))
                    fault_seq = _last_trace_seq(trace_path)
                    _control(control_url, {"type": "inject_failure", "kind": "provider", "reason": fault})
                    time.sleep(float(step.get("post_fault_wait", 2.0)))
                    _control(
                        control_url,
                        {
                            "type": "snapshot",
                            "label": f"{journey_id}-{step_index:02d}-fault",
                            "observation": observation,
                        },
                    )
                    result["ran"] = True
                    _assert_spoken_recovery(trace_path, after_seq=fault_seq)
                elif action == "barge_in":
                    if not _wait_trace_event(
                        trace_path,
                        {"tts.first_audio", "tts.started"},
                        after_seq=last_turn_seq,
                        timeout_s=barge_wait,
                    ):
                        raise E2EError(f"barge-in never observed TTS start within {barge_wait:.1f}s")
                    _control(
                        control_url,
                        {"type": "barge_in", "session_id": f"e2e-{journey_id}", "tenant_id": "t0"},
                    )
                    fixture = _resolve_fixture(base, str(step.get("fixture", "")), fixture_roots)
                    _run_replay(
                        base=base,
                        env=env,
                        run_root=run_root,
                        control_url=control_url,
                        fixture=fixture,
                        journey_id=journey_id,
                        step_index=step_index,
                        session_id=f"e2e-{journey_id}",
                        speed=float(step.get("speed", default_speed)),
                        post_turn_wait=float(step.get("post_turn_wait", default_post_wait)),
                        observation=observation,
                    )
                    result["ran"] = True
                else:
                    raise E2EError(f"unsupported action in {journey_id}[{step_index}]: {action!r}")

            result["status"] = "ran-and-passed"
            result["outcome"] = "ran-and-passed"
            _control(control_url, {"type": "journey_marker", "journey_id": journey_id, "phase": "complete"})
        except (OSError, E2EError, KeyError, TypeError, ValueError) as exc:
            result["error"] = str(exc)
            result["status"] = "could-not-run" if not result["ran"] else _failure_status(str(exc))
            result["outcome"] = result["status"]
            try:
                _control(
                    control_url,
                    {
                        "type": "journey_marker",
                        "journey_id": journey_id,
                        "phase": "failed",
                        "error": str(exc),
                    },
                )
                _control(
                    control_url,
                    {
                        "type": "snapshot",
                        "label": f"{journey_id}-failed",
                        "observation": f"Journey orchestration failed: {exc}",
                    },
                )
            except (OSError, E2EError):
                pass
        results.append(result)
        (run_root / "journey-results.json").write_text(json.dumps(results, indent=2) + "\n", encoding="utf-8")

    return results


def _start(command: Any, *, cwd: Path, env: dict[str, str], log_path: Path) -> subprocess.Popen[Any] | None:
    if not command:
        return None
    if isinstance(command, str):
        raise E2EError("commands must be argv arrays, not shell strings")
    log_path.parent.mkdir(parents=True, exist_ok=True)
    handle = log_path.open("w", encoding="utf-8")
    try:
        process = subprocess.Popen(list(command), cwd=str(cwd), env=env, stdout=handle, stderr=subprocess.STDOUT, text=True)
    except Exception:
        handle.close()
        raise
    handle.close()
    return process


def _url_ok(url: str) -> bool:
    try:
        with urlopen(url, timeout=2) as response:
            return 200 <= response.status < 300
    except Exception:
        return False


def _wait_urls(urls: list[str], timeout_s: float = 30) -> None:
    deadline = time.monotonic() + timeout_s
    while time.monotonic() < deadline:
        if all(_url_ok(url) for url in urls):
            return
        time.sleep(0.25)
    raise E2EError(f"backend readiness failed: {[url for url in urls if not _url_ok(url)]}")


def _terminate(process: subprocess.Popen[Any] | None) -> None:
    if process is None or process.poll() is not None:
        return
    process.terminate()
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=5)


def _run_audio_gate(audio_config: Mapping[str, Any], *, base: Path, run_root: Path, env: dict[str, str]) -> int:
    """Run the model-backed audio gate only when a run opts into it."""
    required = ("stt_audio", "expected_transcript", "tts_audio", "tts_expected_transcript", "nisqa_model")
    missing = [key for key in required if key not in audio_config]
    if missing:
        raise E2EError(f"audio_quality.enabled requires: {', '.join(missing)}")

    def resolve_run_path(value: Any) -> Path:
        return _resolve(base, str(value).replace("{run_root}", str(run_root)))

    audio_report = run_root / "audio-quality.json"
    command = [
        sys.executable,
        "-m",
        "evaluator.audio_quality",
        "--stt-audio",
        str(resolve_run_path(audio_config["stt_audio"])),
        "--expected-transcript",
        str(audio_config["expected_transcript"]),
        "--tts-audio",
        str(resolve_run_path(audio_config["tts_audio"])),
        "--tts-expected-transcript",
        str(audio_config["tts_expected_transcript"]),
        "--nisqa-model",
        str(resolve_run_path(audio_config["nisqa_model"])),
        "--json-out",
        str(audio_report),
    ]
    if audio_config.get("tts_reference_audio"):
        command[command.index("--nisqa-model"):command.index("--nisqa-model")] = [
            "--tts-reference-audio",
            str(resolve_run_path(audio_config["tts_reference_audio"])),
        ]
    if audio_config.get("allow_missing_stoi_reference") is True:
        command.append("--allow-missing-stoi-reference")
    if audio_config.get("whisper_model"):
        command.extend(["--whisper-model", str(audio_config["whisper_model"])])
    if audio_config.get("allow_model_download"):
        command.append("--allow-model-download")
    for key, flag in (("max_wer", "--max-wer"), ("max_cer", "--max-cer"), ("max_tts_wer", "--max-tts-wer"), ("max_tts_cer", "--max-tts-cer"), ("min_stoi", "--min-stoi"), ("min_nisqa_mos", "--min-nisqa-mos")):
        if key in audio_config:
            command.extend([flag, str(audio_config[key])])

    completed = subprocess.run(command, cwd=str(base), env=env, text=True, capture_output=True, check=False)
    (run_root / "audio-quality.log").write_text(completed.stdout + completed.stderr, encoding="utf-8")
    return completed.returncode


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", type=Path, required=True, help="Independent run config")
    parser.add_argument("--wait-seconds", type=int, default=5)
    parser.add_argument("--no-judges", action="store_true")
    args = parser.parse_args(argv)

    config_path = args.config.resolve()
    config = _load(config_path)
    base = config_path.parent
    harness_root = Path(__file__).resolve().parent
    run_id = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    run_root = _resolve(base, str(config.get("run_root", "runs"))) / run_id
    trace_path = run_root / "trace.jsonl"
    screenshots_dir = run_root / "screenshots"
    run_root.mkdir(parents=True, exist_ok=True)
    screenshots_dir.mkdir(parents=True, exist_ok=True)

    env = os.environ.copy()
    env["PYTHONPATH"] = str(harness_root) + os.pathsep + env.get("PYTHONPATH", "")
    owned: list[subprocess.Popen[Any] | None] = []
    proxy_config = dict(config.get("proxy", {}))
    control_url = str(proxy_config.get("control_url", "ws://127.0.0.1:8093"))
    journey_results: list[dict[str, Any]] = []
    try:
        backend = _start(config.get("backend_command"), cwd=base, env=env, log_path=run_root / "backend.log")
        owned.append(backend)
        ready_urls = [str(url) for url in config.get("backend_ready_urls", [])]
        if ready_urls:
            _wait_urls(ready_urls)

        proxy_args = [
            sys.executable, "-m", "transport.ws_proxy",
            "--app-port", str(proxy_config.get("app_port", 8091)),
            "--control-port", str(proxy_config.get("control_port", 8093)),
            "--backend-url", str(proxy_config.get("backend_url", "ws://127.0.0.1:8092")),
            "--trace", str(trace_path), "--run-id", run_id,
            "--tts-audio-dir", str(run_root / "audio"),
            "--ocr-script", str((harness_root / "simulator" / "ocr.swift").resolve()),
            "--turn-timeout-seconds", str(proxy_config.get("turn_timeout_seconds", 30.0)),
        ]
        for expected in config.get("expected_transcripts", []):
            proxy_args.extend(["--expected-transcript", str(expected)])
        proxy_process = _start(proxy_args, cwd=base, env=env, log_path=run_root / "proxy.log")
        owned.append(proxy_process)
        _wait_control(control_url)

        simulator_config_path = _resolve(base, str(config["simulator_config"]))
        simulator = SimulatorRunner(load_config(simulator_config_path), simulator_config_path)
        artifact_dir = simulator.run_session(wait_seconds=args.wait_seconds).resolve()
        launch_result = json.loads((artifact_dir / "launch-result.json").read_text(encoding="utf-8"))
        simulator_udid = str(launch_result["simulator_udid"])
        bundle_id = str(launch_result["app_bundle_id"])
        _control(control_url, {"type": "configure_simulator", "simulator_udid": simulator_udid, "bundle_id": bundle_id, "screenshots_dir": str(screenshots_dir)})
        _ensure_app(control_url, bundle_id=bundle_id, simulator_udid=simulator_udid, timeout_s=60)
        ocr_observation = "launch checkpoint; inspect PNG and OCR"
        ocr_path = Path(str(launch_result.get("ocr_path", "")))
        if not ocr_path.is_file():
            raise E2EError(f"APP_UNHEALTHY: launch OCR artifact is missing: {ocr_path}")
        ocr_payload = json.loads(ocr_path.read_text(encoding="utf-8"))
        if not isinstance(ocr_payload, Mapping):
            raise E2EError("APP_UNHEALTHY: launch OCR payload is not an object")
        _assert_app_healthy(ocr_payload)
        ocr_text = " | ".join(str(item) for item in ocr_payload.get("text", []))
        ocr_observation = f"OCR: {ocr_text}"
        (run_root / "launch.ocr.json").write_text(json.dumps(ocr_payload, indent=2) + "\n", encoding="utf-8")
        log_path = Path(str(launch_result.get("log_path", "")))
        if log_path.exists():
            log_text = log_path.read_text(encoding="utf-8", errors="replace")
            if "mic-start-error" in log_text or "microphone input" in log_text.lower():
                ocr_observation += " | simulator log: microphone initialization error"
            (run_root / "simulator.log").write_text(log_text, encoding="utf-8")
        _control(control_url, {"type": "snapshot", "label": "launch", "observation": ocr_observation})

        plan = _journey_plan(config, base)
        if plan is not None:
            journey_results = _run_journey_plan(
                plan,
                config=config,
                base=base,
                env=env,
                run_root=run_root,
                trace_path=trace_path,
                control_url=control_url,
                bundle_id=bundle_id,
                simulator_udid=simulator_udid,
            )
        else:
            for index, journey in enumerate(config.get("journeys", [])):
                journey_id = str(journey.get("id", f"journey-{index + 1}"))
                result: dict[str, Any] = {
                    "journey": journey_id,
                    "ran": False,
                    "status": "could-not-run",
                    "outcome": "could-not-run",
                    "error": None,
                }
                try:
                    if index:
                        _ensure_app(control_url, bundle_id=bundle_id, simulator_udid=simulator_udid)
                    if journey.get("fault"):
                        fault = journey["fault"]
                        if isinstance(fault, str):
                            fault = {"kind": fault}
                        if not isinstance(fault, Mapping):
                            raise E2EError(f"journey fault must be a string or object: {fault!r}")
                        fault_seq = _last_trace_seq(trace_path)
                        _control(control_url, {"type": "inject_failure", **dict(fault)})
                        result["ran"] = True
                        time.sleep(float(journey.get("post_fault_wait", 2.0)))
                        _control(
                            control_url,
                            {
                                "type": "snapshot",
                                "label": journey_id,
                                "observation": str(
                                    journey.get(
                                        "observation",
                                        "Inspect the spoken failure recovery and visible error state.",
                                    )
                                ),
                            },
                        )
                        _assert_spoken_recovery(trace_path, after_seq=fault_seq)
                    else:
                        fixture = _resolve(base, str(journey["wav"]))
                        result["ran"] = True
                        result["acceptance"] = _run_replay(
                            base=base,
                            env=env,
                            run_root=run_root,
                            control_url=control_url,
                            fixture=fixture,
                            journey_id=journey_id,
                            step_index=index,
                            session_id=str(journey.get("session_id", "deprecated-placeholder")),
                            speed=float(journey.get("speed", 1.0)),
                            post_turn_wait=float(journey.get("post_turn_wait", 2.0)),
                            observation=str(journey.get("observation", "")),
                        )
                    result["status"] = "ran-and-passed"
                    result["outcome"] = "ran-and-passed"
                except (OSError, E2EError, KeyError, TypeError, ValueError) as exc:
                    result["error"] = str(exc)
                    result["status"] = "could-not-run" if not result["ran"] else _failure_status(str(exc))
                    result["outcome"] = result["status"]
                journey_results.append(result)
                (run_root / "journey-results.json").write_text(
                    json.dumps(journey_results, indent=2) + "\n",
                    encoding="utf-8",
                )

        evaluator_args = [sys.executable, "-m", "evaluator", str(trace_path), "--out", str(run_root / "report.md")]
        if not args.no_judges:
            evaluator_args.append("--judges")
        completed = subprocess.run(evaluator_args, cwd=str(base), env=env, text=True, capture_output=True, check=False)
        (run_root / "evaluator.log").write_text(completed.stdout + completed.stderr, encoding="utf-8")
        audio_exit: int | None = None
        audio_config = config.get("audio_quality")
        if isinstance(audio_config, Mapping) and audio_config.get("enabled") is True:
            audio_exit = _run_audio_gate(audio_config, base=base, run_root=run_root, env=env)
        turn_evidence = _turn_evidence(trace_path)
        (run_root / "turn-evidence.json").write_text(
            json.dumps(turn_evidence, indent=2) + "\n",
            encoding="utf-8",
        )
        if plan is not None:
            expected_turns = sum(
                1
                for journey in plan
                for step in journey.get("steps", [])
                if isinstance(step, Mapping) and step.get("action") == "speak_fixture"
            )
        else:
            expected_turns = sum(1 for journey in config.get("journeys", []) if not journey.get("fault"))
        journey_failed = any(item.get("status") != "ran-and-passed" for item in journey_results)
        turn_gate_failed = turn_evidence["turns_complete"] != expected_turns
        tts_lifecycle_failed = turn_evidence["tts_lifecycles_valid"] != expected_turns
        final_exit = completed.returncode if audio_exit in (None, 0) else audio_exit
        if (journey_failed or turn_gate_failed or tts_lifecycle_failed) and final_exit == 0:
            final_exit = 1
        result = {
            "run_id": run_id,
            "run_root": str(run_root),
            "simulator_udid": simulator_udid,
            "evaluator_exit": completed.returncode,
            "audio_quality_exit": audio_exit,
            "journeys_total": len(journey_results),
            "journeys_ran_and_passed": sum(item.get("status") == "ran-and-passed" for item in journey_results),
            "journeys_ran_and_failed": sum(item.get("status") == "ran-and-failed" for item in journey_results),
            "journeys_could_not_run": sum(item.get("status") == "could-not-run" for item in journey_results),
            "turns_expected": expected_turns,
            "turns_complete": turn_evidence["turns_complete"],
            "tts_lifecycles_valid": turn_evidence["tts_lifecycles_valid"],
            "p50_inject_to_first_audio_ms": turn_evidence["p50_inject_to_first_audio_ms"],
        }
        (run_root / "run-result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        print(json.dumps({"run_root": str(run_root), "report": str(run_root / "report.md"), **result}, indent=2))
        return final_exit
    except (OSError, E2EError, KeyError, json.JSONDecodeError) as exc:
        if not journey_results:
            declared = config.get("journeys", [])
            try:
                plan = _journey_plan(config, base)
                if plan is not None:
                    declared = plan
            except (OSError, E2EError, KeyError, json.JSONDecodeError):
                declared = []
            journey_results = [
                {
                    "journey": str(journey.get("id", f"journey-{index + 1}")),
                    "ran": False,
                    "status": "could-not-run",
                    "outcome": "could-not-run",
                    "error": str(exc),
                }
                for index, journey in enumerate(declared)
                if isinstance(journey, Mapping)
            ]
            if journey_results:
                (run_root / "journey-results.json").write_text(
                    json.dumps(journey_results, indent=2) + "\n",
                    encoding="utf-8",
                )
        (run_root / "orchestrator-error.txt").write_text(str(exc) + "\n", encoding="utf-8")
        print(f"[e2e] ERROR: {exc}", file=sys.stderr)
        return 1
    finally:
        for process in reversed(owned):
            _terminate(process)


if __name__ == "__main__":
    raise SystemExit(main())
