#!/usr/bin/env python3
"""Visible, app-agnostic iOS Simulator runner for human-style E2E sessions.

This file deliberately knows nothing about the mobile app implementation. It
only coordinates xcrun simctl, configurable build commands, artifact capture,
and an optional deterministic relay replay command.
"""

from __future__ import annotations

import argparse
import json
import re
import shlex
import signal
import subprocess
import sys
import time
import uuid
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable, Mapping, Sequence


class RunnerError(RuntimeError):
    """Raised when a required simulator operation fails."""


class _RunnerInterrupted(RunnerError):
    """Raised from a signal handler so termination still unwinds through cleanup."""


def _handle_termination_signal(signum: int, frame: Any) -> None:
    raise _RunnerInterrupted(f"runner received signal {signal.Signals(signum).name}")


@dataclass(frozen=True)
class CommandResult:
    args: tuple[str, ...]
    returncode: int
    stdout: str = ""
    stderr: str = ""


DEFAULT_CONFIG: dict[str, Any] = {
    "workspace_root": "../..",
    "command_timeout_seconds": 900,
    "app": {
        "bundle_id": "org.reactjs.native.example.OrbMobile",
        "app_path": "/tmp/focus-orb-xcodebuild/Build/Products/Debug-iphonesimulator/OrbMobile.app",
    },
    "simulator": {
        "create_new": True,
        "device_name": "ADHD Focus Orb E2E",
        "device_type": "com.apple.CoreSimulator.SimDeviceType.iPhone-16-Pro",
        "runtime": "latest",
        "erase_before_boot": True,
        "leave_open": True,
        "show_window": False,
        "gui_app_path": "/Applications/Xcode.app/Contents/Developer/Applications/Simulator.app",
    },
    "permissions": [],
    "commands": {
        "build": ["npm", "run", "ios:build:check"],
        "install": None,
        "open": None,
    },
    "capture": {
        "log_stream": True,
        "log_predicate": "process == 'OrbMobile'",
        "screenshot": True,
        "ocr": False,
        "video": True,
        "wait_seconds": 15,
    },
    "relay_replay": {
        "enabled": False,
        "fixture": "",
        "command": [],
    },
    "evaluators": [],
}


def deep_merge(base: Mapping[str, Any], override: Mapping[str, Any]) -> dict[str, Any]:
    merged: dict[str, Any] = dict(base)
    for key, value in override.items():
        if isinstance(value, Mapping) and isinstance(merged.get(key), Mapping):
            merged[key] = deep_merge(merged[key], value)  # type: ignore[arg-type]
        else:
            merged[key] = value
    return merged


def load_config(path: Path) -> dict[str, Any]:
    with path.open(encoding="utf-8") as handle:
        loaded = json.load(handle)
    if not isinstance(loaded, Mapping):
        raise RunnerError(f"Config must be a JSON object: {path}")
    return deep_merge(DEFAULT_CONFIG, loaded)


def _format_token(value: Any, context: Mapping[str, str]) -> str:
    if not isinstance(value, str):
        raise RunnerError(f"Command arguments must be strings, got {value!r}")
    try:
        return value.format_map(context)
    except KeyError as exc:
        raise RunnerError(f"Unknown command placeholder: {exc.args[0]}") from exc


def command_tokens(command: Any, context: Mapping[str, str]) -> list[str]:
    if command is None or command == []:
        return []
    if isinstance(command, str):
        command = shlex.split(command)
    if not isinstance(command, Sequence) or isinstance(command, (bytes, bytearray)):
        raise RunnerError(f"Command must be an argv array or string: {command!r}")
    return [_format_token(token, context) for token in command]


def _json_or_error(result: CommandResult, label: str) -> Any:
    if result.returncode != 0:
        raise RunnerError(f"{label} failed ({result.returncode}): {result.stderr.strip()}")
    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError as exc:
        raise RunnerError(f"{label} did not return JSON: {exc}") from exc


class SimulatorRunner:
    """Orchestrates one visible run and records all evidence under one folder."""

    def __init__(self, config: Mapping[str, Any], config_path: Path, *, now: Any = None) -> None:
        self.config = config
        self.config_path = config_path
        self.now = now or datetime.now
        self.owned_processes: list[subprocess.Popen[Any]] = []
        self.device_udid: str | None = None
        self.created_device = False
        self.screenshot_path: Path | None = None
        self.ocr_path: Path | None = None
        self.video_path: Path | None = None
        self.app_pid: int | None = None
        self.artifacts_dir: Path | None = None
        self.command_trace: list[dict[str, Any]] = []

    @property
    def workspace_root(self) -> Path:
        configured = Path(str(self.config.get("workspace_root", "../..")))
        if not configured.is_absolute():
            configured = self.config_path.parent / configured
        return configured.resolve()

    def _context(self, **extra: str) -> dict[str, str]:
        app = self.config["app"]
        context = {
            "workspace_root": str(self.workspace_root),
            "artifacts_dir": str(self.artifacts_dir or ""),
            "session_dir": str(self.artifacts_dir or ""),
            "app_path": str(app["app_path"]),
            "bundle_id": str(app["bundle_id"]),
            "udid": str(self.device_udid or ""),
            "fixture": str(self.config.get("relay_replay", {}).get("fixture", "")),
            "replay_handoff": str((self.artifacts_dir / "replay-handoff.json") if self.artifacts_dir else ""),
        }
        context.update(extra)
        return context

    def _record(self, args: Sequence[str], result: CommandResult, *, label: str) -> None:
        self.command_trace.append(
            {
                "label": label,
                "command": list(args),
                "returncode": result.returncode,
                "stdout": result.stdout,
                "stderr": result.stderr,
            }
        )
        if self.artifacts_dir:
            trace_path = self.artifacts_dir / "commands.jsonl"
            with trace_path.open("a", encoding="utf-8") as handle:
                handle.write(json.dumps(self.command_trace[-1], ensure_ascii=False) + "\n")

    def run(self, args: Sequence[str], *, label: str, cwd: Path | None = None) -> CommandResult:
        argv = tuple(args)
        print(f"[simulator] $ {shlex.join(argv)}", flush=True)
        timeout = self.config.get("command_timeout_seconds") or None
        try:
            completed = subprocess.run(
                list(argv),
                cwd=str(cwd or self.workspace_root),
                capture_output=True,
                text=True,
                check=False,
                timeout=timeout,
            )
        except subprocess.TimeoutExpired as exc:
            # A hung build/install/replay/evaluator command must not leave the
            # video/log capture running unbounded: surface it as a RunnerError
            # so `run_session`'s `finally` still stops owned processes.
            #
            # Despite text=True, CPython can hand back the partial buffer collected
            # before the timeout as raw bytes rather than str; decode defensively so
            # this doesn't crash inside exception handling and mask the real error.
            timeout_stdout = exc.stdout.decode("utf-8", "replace") if isinstance(exc.stdout, bytes) else (exc.stdout or "")
            timeout_stderr = exc.stderr.decode("utf-8", "replace") if isinstance(exc.stderr, bytes) else (exc.stderr or "")
            timeout_result = CommandResult(
                argv, -1, timeout_stdout, f"{timeout_stderr.strip()}\ntimed out after {timeout}s".strip()
            )
            self._record(argv, timeout_result, label=label)
            raise RunnerError(f"{label} timed out after {timeout}s and was killed") from exc
        result = CommandResult(argv, completed.returncode, completed.stdout, completed.stderr)
        self._record(argv, result, label=label)
        if result.returncode != 0:
            raise RunnerError(
                f"{label} failed ({result.returncode})\n"
                f"stdout: {result.stdout.strip()}\n"
                f"stderr: {result.stderr.strip()}"
            )
        return result

    def _simctl_json(self, args: Sequence[str], *, label: str) -> Any:
        result = self.run(["xcrun", "simctl", *args], label=label)
        return _json_or_error(result, label)

    def _latest_runtime(self, requested: str) -> str:
        if requested != "latest":
            return requested
        payload = self._simctl_json(["list", "runtimes", "available", "--json"], label="list runtimes")
        runtimes = [item for item in payload.get("runtimes", []) if item.get("isAvailable", True)]
        ios = [item for item in runtimes if str(item.get("identifier", "")).startswith("com.apple.CoreSimulator.SimRuntime.iOS-")]
        if not ios:
            raise RunnerError("No available iOS Simulator runtime found")
        ios.sort(key=lambda item: tuple(int(part) for part in str(item.get("version", "0")).split(".") if part.isdigit()), reverse=True)
        return str(ios[0]["identifier"])

    def _choose_existing_device(self, name: str) -> str:
        payload = self._simctl_json(["list", "devices", "available", "--json"], label="list devices")
        matches: list[dict[str, Any]] = []
        for devices in payload.get("devices", {}).values():
            matches.extend(device for device in devices if device.get("name") == name)
        if not matches:
            raise RunnerError(f"No available simulator named {name!r}")
        return str(matches[0]["udid"])

    def _device_state(self) -> str:
        payload = self._simctl_json(["list", "devices", "available", "--json"], label="read simulator state")
        for devices in payload.get("devices", {}).values():
            for device in devices:
                if device.get("udid") == self.device_udid:
                    return str(device.get("state", ""))
        return ""

    def _select_device(self) -> str:
        simulator = self.config["simulator"]
        name = str(simulator["device_name"])
        if bool(simulator.get("create_new", True)):
            runtime = self._latest_runtime(str(simulator.get("runtime", "latest")))
            result = self.run(
                ["xcrun", "simctl", "create", name, str(simulator["device_type"]), runtime],
                label="create fresh simulator",
            )
            udid = result.stdout.strip().splitlines()[-1]
            if not udid:
                raise RunnerError("simctl create returned an empty UDID")
            self.created_device = True
            return udid
        return self._choose_existing_device(name)

    def _start_capture(self, args: Sequence[str], output_path: Path, *, label: str) -> None:
        output_path.parent.mkdir(parents=True, exist_ok=True)
        handle = output_path.open("w", encoding="utf-8")
        try:
            process = subprocess.Popen(
                list(args),
                cwd=str(self.workspace_root),
                stdout=handle,
                stderr=subprocess.STDOUT,
                text=True,
            )
        except Exception:
            handle.close()
            raise
        # The child owns the descriptor after Popen; close only our parent copy.
        handle.close()
        self.owned_processes.append(process)
        print(f"[simulator] {label}: {output_path}", flush=True)

    def _start_log_stream(self) -> None:
        capture = self.config["capture"]
        if not bool(capture.get("log_stream", True)):
            return
        predicate = str(capture.get("log_predicate", "process == 'OrbMobile'"))
        self._start_capture(
            [
                "xcrun",
                "simctl",
                "spawn",
                str(self.device_udid),
                "log",
                "stream",
                "--style",
                "compact",
                "--level",
                "debug",
                "--predicate",
                predicate,
            ],
            self.artifacts_dir / "simulator.log",  # type: ignore[operator]
            label="log capture",
        )

    def _start_video_capture(self) -> None:
        if not bool(self.config["capture"].get("video", True)):
            return
        if "recordVideo" not in self._simctl_io_help():
            print("[simulator] video capture unavailable; continuing without video", flush=True)
            return
        self.video_path = self.artifacts_dir / "session.mp4"  # type: ignore[operator]
        self._start_capture(
            ["xcrun", "simctl", "io", str(self.device_udid), "recordVideo", str(self.video_path.resolve())],
            self.artifacts_dir / "video.log",  # type: ignore[operator]
            label="video capture",
        )

    def _simctl_io_help(self) -> str:
        result = self.run(["xcrun", "simctl", "help", "io"], label="inspect simctl io capabilities")
        # Current Xcode prints this help to stderr on some releases and stdout
        # on others.  Capability detection must inspect both streams or the
        # evidence capture silently disappears.
        return f"{result.stdout}\n{result.stderr}"

    def _stop_owned_processes(self) -> None:
        for process in reversed(self.owned_processes):
            try:
                # `xcrun simctl io ... recordVideo` (and `log stream`) are designed to
                # be stopped like an interactive Ctrl-C: SIGINT tells the CoreSimulator
                # daemon to finalize and release the file. SIGTERM only kills this CLI
                # wrapper process — the daemon keeps recording to the file indefinitely
                # regardless, which is what produced a 69GB orphaned session.mp4 in this
                # repo (confirmed: after SIGTERM, `lsof` still showed SimRenderServer
                # holding the file, and the daemon refused a new recording as "already
                # in progress"). Escalate to SIGTERM/SIGKILL only if the graceful stop
                # doesn't finish in time.
                process.send_signal(signal.SIGINT)
                process.wait(timeout=10)
            except ProcessLookupError:
                continue
            except subprocess.TimeoutExpired:
                try:
                    process.terminate()
                    process.wait(timeout=5)
                except (ProcessLookupError, subprocess.TimeoutExpired):
                    try:
                        process.kill()
                    except ProcessLookupError:
                        pass
        self.owned_processes.clear()

    def _capture_screenshot(self) -> None:
        if bool(self.config["capture"].get("screenshot", True)) and "screenshot" in self._simctl_io_help():
            self.screenshot_path = self.artifacts_dir / "launch.png"  # type: ignore[operator]
            self.run(
                ["xcrun", "simctl", "io", str(self.device_udid), "screenshot", str(self.screenshot_path.resolve())],
                label="launch screenshot",
            )

    def _capture_ocr(self) -> None:
        if not bool(self.config["capture"].get("ocr", False)) or self.screenshot_path is None:
            return
        script = Path(__file__).with_name("ocr.swift")
        result = self.run(["swift", str(script), str(self.screenshot_path.resolve())], label="OCR launch screenshot")
        self.ocr_path = self.artifacts_dir / "launch.ocr.json"  # type: ignore[operator]
        try:
            payload = json.loads(result.stdout)
        except json.JSONDecodeError as exc:
            raise RunnerError(f"OCR did not return JSON: {exc}") from exc
        self.ocr_path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")

    def _launch_app(self) -> None:
        configured = command_tokens(self.config["commands"].get("open"), self._context())
        if configured:
            result = self.run(configured, label="open app")
        else:
            result = self.run(
                ["xcrun", "simctl", "launch", str(self.device_udid), str(self.config["app"]["bundle_id"])],
                label="launch app",
            )
        # simctl commonly prints either "...: pid 123" or "...: 123". A
        # custom open command may not expose a PID, so the marker is explicit
        # about that instead of guessing.
        pid_matches = re.findall(r"(?:\bpid\s*)?(\d+)\b", result.stdout, flags=re.IGNORECASE)
        self.app_pid = int(pid_matches[-1]) if pid_matches else None

    def _show_simulator_window(self) -> None:
        simulator = self.config["simulator"]
        if not bool(simulator.get("show_window", False)):
            return
        self.run(["open", "-a", str(simulator.get("gui_app_path", "Simulator"))], label="show simulator window")

    def _write_launch_marker(self) -> None:
        marker = {
            "simulator_udid": self.device_udid,
            "app_bundle_id": self.config["app"]["bundle_id"],
            "app_pid": self.app_pid,
            "screenshot_path": str(self.screenshot_path) if self.screenshot_path else None,
            "ocr_path": str(self.ocr_path) if self.ocr_path else None,
            "log_path": str(self.artifacts_dir / "simulator.log") if self.config["capture"].get("log_stream", True) else None,
            "video_path": str(self.video_path) if self.video_path else None,
            "artifacts_dir": str(self.artifacts_dir),
        }
        (self.artifacts_dir / "launch-result.json").write_text(json.dumps(marker, indent=2) + "\n", encoding="utf-8")
        print("=== SIMULATOR_LAUNCH_RESULT ===", flush=True)
        print(json.dumps(marker, sort_keys=True), flush=True)

    def _write_replay_handoff(self) -> Path:
        replay = self.config.get("relay_replay", {})
        fixture = Path(str(replay.get("fixture", "")))
        if not fixture.is_absolute():
            fixture = self.config_path.parent / fixture
        handoff = {
            "schema_version": 1,
            "kind": "deterministic_microphone_replay_handoff",
            "fixture_path": str(fixture.resolve()),
            "simulator_udid": self.device_udid,
            "app_bundle_id": self.config["app"]["bundle_id"],
            "artifacts_dir": str(self.artifacts_dir),
            "contract": {
                "input": "WAV or PCM fixture, converted by the replay process into the relay wire format",
                "transport": "The configured replay command sends frames to the relay/backend; this runner does not inject host audio into CoreSimulator.",
                "timing": "Replay must preserve or explicitly declare frame duration and sequence numbers.",
                "evidence": "Replay should write transcript, backend response, frame timing, and errors under artifacts_dir.",
                "ownership": "The runner owns only the simulator and its own capture subprocesses; replay owns its own process.",
            },
        }
        path = self.artifacts_dir / "replay-handoff.json"
        path.write_text(json.dumps(handoff, indent=2) + "\n", encoding="utf-8")
        return path

    def _run_configured(self, name: str, *, cwd: Path | None = None, **extra: str) -> None:
        command = command_tokens(self.config["commands"].get(name), self._context(**extra))
        if command:
            self.run(command, label=name, cwd=cwd)

    def _install_app(self) -> None:
        configured = command_tokens(self.config["commands"].get("install"), self._context())
        if configured:
            self.run(configured, label="install")
            return
        self.run(
            ["xcrun", "simctl", "install", str(self.device_udid), str(self.config["app"]["app_path"])],
            label="install app",
        )

    def _grant_permissions(self) -> None:
        """Apply only permissions explicitly requested by the test manifest."""
        for service in self.config.get("permissions", []):
            self.run(
                ["xcrun", "simctl", "privacy", str(self.device_udid), "grant", str(service), str(self.config["app"]["bundle_id"])],
                label=f"grant {service} permission",
            )

    def _run_replay(self) -> None:
        replay = self.config.get("relay_replay", {})
        if not bool(replay.get("enabled", False)):
            return
        fixture = str(replay.get("fixture", ""))
        if not fixture:
            raise RunnerError("relay_replay.enabled is true but fixture is empty")
        fixture_path = Path(fixture)
        if not fixture_path.is_absolute():
            fixture_path = self.config_path.parent / fixture_path
        self._write_replay_handoff()
        command = command_tokens(
            replay.get("command"),
            self._context(fixture=str(fixture_path.resolve())),
        )
        if not command:
            raise RunnerError("relay_replay.enabled is true but command is empty")
        self.run(command, label="deterministic relay replay")

    def _run_evaluators(self) -> None:
        for evaluator in self.config.get("evaluators", []):
            if not isinstance(evaluator, Mapping) or not evaluator.get("enabled", False):
                continue
            name = str(evaluator.get("name", "evaluator"))
            command = command_tokens(evaluator.get("command"), self._context())
            if command:
                self.run(command, label=f"evaluator:{name}")

    def _write_report(self, *, started_at: str, status: str, error: str | None = None) -> None:
        report = self.artifacts_dir / "report.md"  # type: ignore[operator]
        lines = [
            "# Human simulator run",
            "",
            f"- Status: **{status}**",
            f"- Started: `{started_at}`",
            f"- Simulator UDID: `{self.device_udid or 'not selected'}`",
            f"- Config: `{self.config_path}`",
            f"- Simulator left open: `{self.config['simulator'].get('leave_open', True)}`",
            "",
            "## Evidence",
            "",
            "- `commands.jsonl`: every synchronous command and its output.",
            "- `simulator.log`: simulator log stream when supported.",
            "- `launch.png`: post-launch screenshot when supported.",
            "- `session.mp4`: visible session recording when supported.",
            "- `relay-replay.*` or evaluator outputs: optional deterministic/evaluator evidence.",
            "",
            "## Human review prompts",
            "",
            "- Did the orb visibly enter listening, working, speaking, success, and error states?",
            "- Was the response understandable, relevant, non-repeating, and appropriately energetic?",
            "- Did the first audio response arrive within the configured latency target?",
            "- Did any failure produce a spoken, actionable message?",
            "- Did the replayed transcript and backend trace agree with what was said?",
        ]
        if error:
            lines.extend(["", "## Runner failure", "", f"```text\n{error}\n```"])
        report.write_text("\n".join(lines) + "\n", encoding="utf-8")

    def run_session(self, *, wait_seconds: int | None = None) -> Path:
        started_at = self.now(timezone.utc).isoformat()
        run_id = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ") + "-" + uuid.uuid4().hex[:8]
        self.artifacts_dir = Path(str(self.config.get("artifacts_root", "artifacts"))) / run_id
        if not self.artifacts_dir.is_absolute():
            self.artifacts_dir = self.config_path.parent / self.artifacts_dir
        self.artifacts_dir.mkdir(parents=True, exist_ok=True)
        status = "failed"
        error: str | None = None
        # SIGINT already becomes KeyboardInterrupt (Python's default handler), which
        # still unwinds through `finally` below. SIGTERM/SIGHUP do not by default: the
        # process would exit immediately and leave the video/log capture subprocess
        # (and the disk it's writing to) running with nothing left to stop it. Convert
        # them into a RunnerError so a cancelled or terminated run still cleans up.
        previous_handlers = {
            sig: signal.signal(sig, _handle_termination_signal) for sig in (signal.SIGTERM, signal.SIGHUP)
        }
        try:
            self._run_configured("build", cwd=self.workspace_root)
            self.device_udid = self._select_device()
            simulator = self.config["simulator"]
            # Existing devices are never shut down or erased implicitly. This
            # keeps the runner safe when a user explicitly points it at a
            # simulator that belongs to another workflow.
            if self.created_device and bool(simulator.get("erase_before_boot", True)):
                # `simctl create` returns a Shutdown device. Calling shutdown on it is a
                # non-zero error on current CoreSimulator, so erase directly; this device was
                # created by this run and is therefore safe to reset.
                self.run(["xcrun", "simctl", "erase", str(self.device_udid)], label="erase selected simulator")
            if not self.created_device and self._device_state().lower() == "booted":
                print(f"[simulator] simulator already booted: {self.device_udid}", flush=True)
            else:
                self.run(["xcrun", "simctl", "boot", str(self.device_udid)], label="boot simulator")
            self.run(["xcrun", "simctl", "bootstatus", str(self.device_udid), "-b"], label="wait for simulator boot")
            self._install_app()
            self._grant_permissions()
            self._show_simulator_window()
            self._start_log_stream()
            self._start_video_capture()
            self._launch_app()
            self._capture_screenshot()
            self._capture_ocr()
            self._write_launch_marker()
            self._run_replay()
            sleep_for = wait_seconds if wait_seconds is not None else int(self.config["capture"].get("wait_seconds", 15))
            if sleep_for > 0:
                print(f"[simulator] visible session; waiting {sleep_for}s", flush=True)
                time.sleep(sleep_for)
            self._run_evaluators()
            status = "passed"
        except (OSError, RunnerError) as exc:
            error = str(exc)
            print(f"[simulator] ERROR: {error}", file=sys.stderr, flush=True)
        finally:
            for sig, handler in previous_handlers.items():
                signal.signal(sig, handler)
            self._stop_owned_processes()
            self._write_report(started_at=started_at, status=status, error=error)
            if self.device_udid and bool(self.config["simulator"].get("leave_open", True)):
                print(f"[simulator] leaving simulator open: {self.device_udid}", flush=True)
        if error:
            raise RunnerError(error)
        return self.artifacts_dir


def parse_args(argv: Iterable[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", type=Path, default=Path(__file__).with_name("manifest.json"))
    parser.add_argument("--wait-seconds", type=int, default=None)
    parser.add_argument("--no-video", action="store_true")
    parser.add_argument("--no-screenshot", action="store_true")
    parser.add_argument("--no-replay", action="store_true")
    return parser.parse_args(argv)


def main(argv: Iterable[str] | None = None) -> int:
    args = parse_args(argv)
    config = load_config(args.config)
    if args.no_video:
        config["capture"]["video"] = False
    if args.no_screenshot:
        config["capture"]["screenshot"] = False
    if args.no_replay:
        config["relay_replay"]["enabled"] = False
    try:
        artifacts = SimulatorRunner(config, args.config).run_session(wait_seconds=args.wait_seconds)
    except RunnerError:
        return 1
    print(f"[simulator] artifacts: {artifacts}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
