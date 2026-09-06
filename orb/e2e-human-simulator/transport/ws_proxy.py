#!/usr/bin/env python3
"""WebSocket MITM for driving the real app connection without app changes.

The simulator app connects to the public port.  This proxy owns that port,
opens a second connection to relay-rs, forwards the app's frames, and exposes
a separate control WebSocket that can inject deterministic PCM frames into the
same backend session.  That is the important distinction from a backend-only
test: transcripts and TTS return through the app's real connection and are
therefore visible in the running simulator.
"""

from __future__ import annotations

import argparse
import asyncio
from dataclasses import dataclass, field
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import signal
import subprocess
import time
from typing import Any
import wave

import websockets
from websockets.server import ServerConnection


APP_HEALTH_ERROR_PATTERNS = (
    "unhandled js exception",
    "invariant violation",
    "referenceerror",
    "typeerror",
    "syntaxerror",
    "unable to load script",
    "could not connect to development server",
    "something went wrong",
    "react native redbox",
)
APP_HEALTH_ALLOWED_TEXT = ("mic error - check console", "mic error — check console")


def _now() -> str:
    return datetime.now(timezone.utc).isoformat()


def _json(value: str | bytes) -> dict[str, Any] | None:
    if not isinstance(value, str):
        return None
    try:
        decoded = json.loads(value)
    except json.JSONDecodeError:
        return None
    return decoded if isinstance(decoded, dict) else None


@dataclass
class TraceWriter:
    path: Path
    run_id: str
    seq: int = 0
    handle: Any = field(init=False, repr=False)

    def __post_init__(self) -> None:
        self.path.parent.mkdir(parents=True, exist_ok=True)
        self.handle = self.path.open("a", encoding="utf-8")

    def emit(
        self,
        source: str,
        event: str,
        data: dict[str, Any],
        *,
        session_id: str | None = None,
    ) -> dict[str, Any] | None:
        if self.handle.closed:
            return None
        self.seq += 1
        record = {
            "ts": _now(),
            "seq": self.seq,
            "source": source,
            "event": event,
            "run_id": self.run_id,
            "session_id": session_id,
            "data": data,
        }
        self.handle.write(json.dumps(record, ensure_ascii=False, sort_keys=True) + "\n")
        self.handle.flush()
        return record

    def close(self) -> None:
        if not self.handle.closed:
            self.handle.close()


@dataclass
class ProxyState:
    backend_url: str
    trace: TraceWriter
    app: ServerConnection | None = None
    backend: Any = None
    session_id: str | None = None
    tenant_id: str | None = None
    suppress_app_audio: bool = True
    tts_audio_seen: bool = False
    boot_state_emitted: bool = False
    simulator_udid: str | None = None
    screenshots_dir: Path | None = None
    app_bundle_id: str | None = None
    tts_audio_dir: Path | None = None
    tts_audio_index: int = 0
    tts_audio: bytearray = field(default_factory=bytearray)
    connection_generation: int = 0
    identity_generation: int = 0
    active_turn: dict[str, Any] | None = None
    turn_counter: int = 0
    tts_started_monotonic: float | None = None
    last_checkpoint_sha256: str | None = None
    last_checkpoint_label: str | None = None
    last_checkpoint_center_rgb: tuple[float, float, float] | None = None
    ocr_script: Path | None = None
    turn_timeout_seconds: float = 30.0
    allow_identity_mismatch_test: bool = False
    lock: asyncio.Lock = field(default_factory=asyncio.Lock)

    @property
    def connected(self) -> bool:
        return self.app is not None and self.backend is not None

    def clear_app_identity(self) -> None:
        self.session_id = None
        self.tenant_id = None
        self.identity_generation += 1

    def observe_app_identity(self, payload: dict[str, Any]) -> None:
        tenant_id = payload.get("tenant_id")
        session_id = payload.get("session_id")
        if not isinstance(tenant_id, str) or not tenant_id:
            return
        if not isinstance(session_id, str) or not session_id:
            return
        if (tenant_id, session_id) == (self.tenant_id, self.session_id):
            return
        previous = {"tenant_id": self.tenant_id, "session_id": self.session_id}
        self.tenant_id = tenant_id
        self.session_id = session_id
        self.identity_generation += 1
        self.trace.emit(
            "harness",
            "app.identity.observed",
            {
                "previous": previous,
                "current": {"tenant_id": tenant_id, "session_id": session_id},
                "identity_generation": self.identity_generation,
            },
            session_id=session_id,
        )

    def current_identity(self) -> tuple[str, str]:
        if not self.tenant_id or not self.session_id:
            raise RuntimeError(
                "APP_IDENTITY_UNAVAILABLE: proxy has not observed tenant_id/session_id "
                "from the app; refusing placeholder injection"
            )
        return self.tenant_id, self.session_id

    def injected_frame(self, payload: dict[str, Any]) -> tuple[dict[str, Any], tuple[str, str]]:
        live_identity = self.current_identity()
        frame = dict(payload)
        frame.pop("test_identity_override", None)
        injected_identity = live_identity
        override = payload.get("test_identity_override")
        if override is not None:
            if not self.allow_identity_mismatch_test:
                raise RuntimeError("identity overrides are disabled outside the negative-control test")
            if not isinstance(override, dict):
                raise RuntimeError("test_identity_override must be an object")
            injected_identity = (
                str(override.get("tenant_id", live_identity[0])),
                str(override.get("session_id", live_identity[1])),
            )
        frame["tenant_id"], frame["session_id"] = injected_identity
        return frame, live_identity

    def begin_turn(self, frame: dict[str, Any], live_identity: tuple[str, str]) -> dict[str, Any]:
        if self.active_turn is not None:
            raise RuntimeError("TURN_ALREADY_ACTIVE: await the current injected turn before starting another")
        self.turn_counter += 1
        turn_id = f"injected-{self.turn_counter:03d}"
        started = time.monotonic()
        record = self.trace.emit(
            "harness",
            "inject.end_of_turn",
            {
                **frame,
                "turn_id": turn_id,
                "identity_source": (
                    "app_observed"
                    if (frame["tenant_id"], frame["session_id"]) == live_identity
                    else "negative_control_override"
                ),
                "identity_generation": self.identity_generation,
            },
            session_id=live_identity[1],
        )
        self.active_turn = {
            "turn_id": turn_id,
            "tenant_id": live_identity[0],
            "session_id": live_identity[1],
            "identity_generation": self.identity_generation,
            "started_monotonic": started,
            "inject_ts": None if record is None else record["ts"],
            "inject_seq": None if record is None else record["seq"],
            "transcript": None,
            "assistant_response": None,
            "tts_started": None,
            "tts_first_audio": None,
            "tts_complete": None,
        }
        return self.active_turn

    def observe_turn_event(self, event: str, payload: dict[str, Any]) -> None:
        turn = self.active_turn
        if turn is None:
            return
        payload_session = payload.get("session_id") or self.session_id
        payload_tenant = payload.get("tenant_id") or self.tenant_id
        if (payload_tenant, payload_session) != (turn["tenant_id"], turn["session_id"]):
            return
        if event == "stt.final" and turn["transcript"] is None:
            turn["transcript"] = dict(payload)
        elif (
            event == "assistant.response"
            and turn["transcript"] is not None
            and turn["assistant_response"] is None
        ):
            turn["assistant_response"] = dict(payload)
        elif (
            event == "tts.started"
            and turn["assistant_response"] is not None
            and turn["tts_started"] is None
        ):
            turn["tts_started"] = dict(payload)
        elif (
            event == "tts.first_audio"
            and turn["tts_started"] is not None
            and turn["tts_first_audio"] is None
        ):
            turn["tts_first_audio"] = dict(payload)
            turn["first_audio_monotonic"] = time.monotonic()
        elif (
            event == "tts.complete"
            and turn["tts_first_audio"] is not None
            and turn["tts_complete"] is None
        ):
            turn["tts_complete"] = dict(payload)

    async def await_turn_stage(self, turn_id: str, stage: str) -> dict[str, Any]:
        if stage not in {
            "transcript",
            "assistant_response",
            "tts_started",
            "tts_first_audio",
            "tts_complete",
        }:
            raise RuntimeError(f"unknown turn stage: {stage}")
        deadline = time.monotonic() + self.turn_timeout_seconds
        while time.monotonic() < deadline:
            turn = self.active_turn
            if turn is None or turn["turn_id"] != turn_id:
                raise RuntimeError(f"unknown active turn: {turn_id}")
            if turn["identity_generation"] != self.identity_generation:
                self.active_turn = None
                raise RuntimeError(
                    "APP_IDENTITY_CHANGED: app session identity changed during injection; "
                    "turn was aborted instead of mixing sessions"
                )
            if turn[stage] is not None:
                return {
                    "type": "turn_stage_observed",
                    "turn_id": turn_id,
                    "stage": stage,
                    "tenant_id": turn["tenant_id"],
                    "session_id": turn["session_id"],
                }
            await asyncio.sleep(0.01)
        raise RuntimeError(
            f"TURN_INCOMPLETE: injected turn {turn_id} timed out after "
            f"{self.turn_timeout_seconds:.1f}s waiting for {stage}"
        )

    async def await_turn(self, turn_id: str) -> dict[str, Any]:
        deadline = time.monotonic() + self.turn_timeout_seconds
        while time.monotonic() < deadline:
            turn = self.active_turn
            if turn is None or turn["turn_id"] != turn_id:
                raise RuntimeError(f"unknown active turn: {turn_id}")
            if turn["identity_generation"] != self.identity_generation:
                self.active_turn = None
                raise RuntimeError(
                    "APP_IDENTITY_CHANGED: app session identity changed during injection; "
                    "turn was aborted instead of mixing sessions"
                )
            missing = [
                field
                for field in (
                    "transcript",
                    "assistant_response",
                    "tts_started",
                    "tts_first_audio",
                    "tts_complete",
                )
                if turn[field] is None
            ]
            if not missing:
                latency_ms = (
                    float(turn["first_audio_monotonic"]) - float(turn["started_monotonic"])
                ) * 1000.0
                accepted = {
                    "type": "turn_accepted",
                    "turn_id": turn_id,
                    "tenant_id": turn["tenant_id"],
                    "session_id": turn["session_id"],
                    "transcript": turn["transcript"],
                    "assistant_response": turn["assistant_response"],
                    "tts_started": turn["tts_started"],
                    "tts_first_audio": turn["tts_first_audio"],
                    "tts_complete": turn["tts_complete"],
                    "inject_to_first_audio_ms": round(latency_ms, 3),
                }
                self.trace.emit(
                    "harness",
                    "turn.accepted",
                    accepted,
                    session_id=turn["session_id"],
                )
                self.active_turn = None
                return accepted
            await asyncio.sleep(0.01)
        turn = self.active_turn
        missing = [] if turn is None else [
            field
            for field in (
                "transcript",
                "assistant_response",
                "tts_started",
                "tts_first_audio",
                "tts_complete",
            )
            if turn[field] is None
        ]
        self.active_turn = None
        code = "RELAY_TURN_NOT_ACCEPTED" if "transcript" in missing else "TURN_INCOMPLETE"
        raise RuntimeError(
            f"{code}: injected turn {turn_id} timed out after {self.turn_timeout_seconds:.1f}s; "
            f"missing={','.join(missing)}. Proxy-generated acks are not relay acceptance."
        )

    def snapshot(self, label: str, observation: str = "") -> dict[str, Any]:
        if not self.simulator_udid or self.screenshots_dir is None:
            raise RuntimeError("proxy was not configured with --simulator-udid and --screenshots-dir")
        safe_label = re.sub(r"[^A-Za-z0-9._-]+", "-", label).strip("-") or "checkpoint"
        path = self.screenshots_dir / f"{self.trace.run_id}-{safe_label}.png"
        path.parent.mkdir(parents=True, exist_ok=True)
        completed = subprocess.run(
            ["xcrun", "simctl", "io", self.simulator_udid, "screenshot", str(path)],
            capture_output=True,
            text=True,
            check=False,
        )
        if completed.returncode != 0:
            raise RuntimeError(completed.stderr.strip() or "simctl screenshot failed")
        screenshot_sha256 = hashlib.sha256(path.read_bytes()).hexdigest()
        duplicate_of = self.last_checkpoint_label if self.last_checkpoint_sha256 == screenshot_sha256 else None

        ocr_payload: dict[str, Any] = {"text": []}
        ocr_path: Path | None = None
        if self.ocr_script is not None:
            ocr = subprocess.run(
                ["swift", str(self.ocr_script), str(path.resolve())],
                capture_output=True,
                text=True,
                check=False,
            )
            if ocr.returncode != 0:
                raise RuntimeError(f"checkpoint OCR failed: {ocr.stderr.strip()}")
            try:
                decoded = json.loads(ocr.stdout)
            except json.JSONDecodeError as exc:
                raise RuntimeError(f"checkpoint OCR returned invalid JSON: {exc}") from exc
            if not isinstance(decoded, dict):
                raise RuntimeError("checkpoint OCR returned a non-object payload")
            ocr_payload = decoded
            ocr_path = path.with_suffix(".ocr.json")
            ocr_path.write_text(json.dumps(ocr_payload, indent=2) + "\n", encoding="utf-8")

        ocr_text = [str(item) for item in ocr_payload.get("text", [])]
        normalized_ocr = " ".join(ocr_text).lower()
        allowed_health_text = [
            allowed for allowed in APP_HEALTH_ALLOWED_TEXT if allowed in normalized_ocr
        ]
        for allowed in APP_HEALTH_ALLOWED_TEXT:
            normalized_ocr = normalized_ocr.replace(allowed, "")
        health_errors = [pattern for pattern in APP_HEALTH_ERROR_PATTERNS if pattern in normalized_ocr]

        center_rgb: tuple[float, float, float] | None = None
        raw_center_rgb = ocr_payload.get("center_rgb")
        if (
            isinstance(raw_center_rgb, list)
            and len(raw_center_rgb) == 3
            and all(isinstance(value, (int, float)) for value in raw_center_rgb)
        ):
            center_rgb = tuple(float(value) for value in raw_center_rgb)
        center_delta = None
        if center_rgb is not None and self.last_checkpoint_center_rgb is not None:
            center_delta = sum(
                (current - previous) ** 2
                for current, previous in zip(center_rgb, self.last_checkpoint_center_rgb)
            ) ** 0.5

        state_hint = None
        for suffix, state in (
            ("-listening", "listening"),
            ("-processing", "processing"),
            ("-complete", "idle"),
            ("-speaking", "speaking"),
            ("-launch", "idle"),
        ):
            if label == "launch" or label.endswith(suffix):
                state_hint = state
                break
        ocr_confirms_state = state_hint is not None and state_hint in normalized_ocr
        visual_confirms_transition = center_delta is not None and center_delta >= 8.0
        ui_confirmed = bool(ocr_confirms_state or visual_confirms_transition)
        self.trace.emit(
            "simulator",
            "ui.screenshot",
            {
                "label": label,
                "path": str(path),
                "sha256": screenshot_sha256,
                "observation": observation,
                "ocr_path": None if ocr_path is None else str(ocr_path),
                "ocr_text": ocr_text,
                "center_rgb": center_rgb,
                "center_rgb_delta": center_delta,
                "ui_confirmed": ui_confirmed,
                "confirmation_basis": (
                    "ocr" if ocr_confirms_state else "center_color_delta" if visual_confirms_transition else None
                ),
                "known_simulator_mic_error": bool(allowed_health_text),
            },
            session_id=self.session_id,
        )
        if state_hint is not None:
            self.trace.emit(
                "simulator",
                "orb.state",
                {
                    "state": state_hint,
                    "basis": f"checkpoint:{label}",
                    "ui_confirmed": ui_confirmed,
                    "confirmation_basis": (
                        "ocr" if ocr_confirms_state else "center_color_delta" if visual_confirms_transition else None
                    ),
                    "ocr_text": ocr_text,
                    "center_rgb_delta": center_delta,
                },
                session_id=self.session_id,
            )
        self.last_checkpoint_sha256 = screenshot_sha256
        self.last_checkpoint_label = label
        self.last_checkpoint_center_rgb = center_rgb
        if health_errors:
            self.trace.emit(
                "harness",
                "app.health.failed",
                {"label": label, "errors": health_errors, "ocr_text": ocr_text},
                session_id=self.session_id,
            )
            raise RuntimeError(
                f"APP_UNHEALTHY: checkpoint {label!r} contains app/RN error text: {health_errors}"
            )
        if duplicate_of is not None:
            self.trace.emit(
                "harness",
                "ui.stuck",
                {"label": label, "duplicate_of": duplicate_of, "sha256": screenshot_sha256},
                session_id=self.session_id,
            )
            raise RuntimeError(
                f"DUPLICATE_CHECKPOINT: {label!r} is byte-identical to consecutive checkpoint "
                f"{duplicate_of!r}"
            )
        return {
            "path": str(path),
            "sha256": screenshot_sha256,
            "state": state_hint,
            "ui_confirmed": ui_confirmed,
            "confirmation_basis": (
                "ocr" if ocr_confirms_state else "center_color_delta" if visual_confirms_transition else None
            ),
        }

    async def send_backend(self, payload: str | bytes) -> None:
        if self.backend is None:
            raise RuntimeError("no app backend session is connected")
        await self.backend.send(payload)

    def start_tts_capture(self) -> None:
        self.tts_audio.clear()

    def append_tts_audio(self, payload: bytes) -> None:
        self.tts_audio.extend(payload)

    def finish_tts_capture(self) -> None:
        if self.tts_audio_dir is None or not self.tts_audio:
            return
        self.tts_audio_dir.mkdir(parents=True, exist_ok=True)
        self.tts_audio_index += 1
        path = self.tts_audio_dir / f"assistant-{self.tts_audio_index:03d}.wav"
        with wave.open(str(path), "wb") as output:
            output.setnchannels(1)
            output.setsampwidth(2)
            output.setframerate(24_000)
            output.writeframes(bytes(self.tts_audio))
        self.trace.emit(
            "harness",
            "tts.audio.captured",
            {"path": str(path), "bytes": len(self.tts_audio), "sample_rate_hz": 24_000},
            session_id=self.session_id,
        )
        self.tts_audio.clear()

    def record_json(self, source: str, payload: dict[str, Any]) -> None:
        message_type = str(payload.get("type", "unknown"))
        session_id = payload.get("session_id") or self.session_id
        event = {
            "transcript": "stt.partial",
            "speech_starting": "tts.audio_stream.started",
            "speech_complete": "tts.complete",
            "closing": "relay.closing",
        }.get(message_type, f"relay.{message_type}")
        trace_payload = dict(payload)
        if message_type == "speech_starting":
            self.tts_audio_seen = False
            self.start_tts_capture()
            trace_payload.update(
                {
                    "provider_streaming": False,
                    "transport_streaming": True,
                    "timing_scope": "relay_audio_stream_marker_after_synchronous_synthesis",
                }
            )
        if message_type == "speech_complete":
            self.finish_tts_capture()
            trace_payload.update(
                {
                    "provider_streaming": False,
                    "transport_streaming": True,
                    "completion_scope": "relay_audio_transfer_complete_not_device_playback",
                    "elapsed_since_tts_request_ms": (
                        None
                        if self.tts_started_monotonic is None
                        else round((time.monotonic() - self.tts_started_monotonic) * 1000.0, 3)
                    ),
                }
            )
        if message_type == "transcript" and payload.get("is_final"):
            event = "stt.final"
        if message_type == "closing" and str(payload.get("reason", "")).lower() in {"provider_failure", "error"}:
            event = "failure"
        if self.active_turn is not None and event in {"stt.final", "tts.audio_stream.started", "tts.complete"}:
            trace_payload["turn_id"] = self.active_turn["turn_id"]
        self.trace.emit(
            "relay-rs",
            event,
            trace_payload,
            session_id=session_id if isinstance(session_id, str) else None,
        )
        self.observe_turn_event(event, trace_payload)
        inferred_state = {
            "transcript": "processing",
            "speech_starting": "speaking",
            "speech_complete": "idle",
            "closing": "error" if event == "failure" else "closed",
        }.get(message_type)
        if inferred_state:
            self.trace.emit(
                "harness.inferred",
                "orb.state",
                {"state": inferred_state, "basis": f"relay.{message_type}", "ui_confirmed": False},
                session_id=session_id if isinstance(session_id, str) else None,
            )


async def _app_to_backend(state: ProxyState, app: ServerConnection) -> None:
    async for message in app:
        if isinstance(message, bytes):
            state.trace.emit(
                "app",
                "mic.frame.suppressed" if state.suppress_app_audio else "mic.frame",
                {"bytes": len(message), "sha256": hashlib.sha256(message).hexdigest()},
                session_id=state.session_id,
            )
            if not state.suppress_app_audio:
                await state.send_backend(message)
            continue
        payload = _json(message)
        if payload:
            state.observe_app_identity(payload)
            message_type = str(payload.get("type", "control"))
            event = "assistant.response" if message_type == "speak" else f"app.{message_type}"
            trace_payload = dict(payload)
            if message_type == "speak" and isinstance(payload.get("text"), str):
                spoken = str(payload["text"])
                trace_payload["speech"] = {
                    "spoken_text": spoken,
                    "plain_text": re.sub(r"\[[^\]]+\]", "", spoken).strip(),
                }
            if message_type == "speak" and state.active_turn is not None:
                trace_payload["turn_id"] = state.active_turn["turn_id"]
            state.trace.emit("app", event, trace_payload, session_id=state.session_id)
            state.observe_turn_event(event, trace_payload)
            if message_type == "start_listening":
                state.trace.emit(
                    "harness.inferred",
                    "orb.state",
                    {"state": "listening", "basis": "app.start_listening", "ui_confirmed": False},
                    session_id=state.session_id,
                )
            elif message_type == "speak":
                state.tts_started_monotonic = time.monotonic()
                tts_started = {
                    "session_id": state.session_id,
                    "tenant_id": state.tenant_id,
                    "provider_streaming": False,
                    "transport_streaming": True,
                    "timing_scope": "synthesis_request_forwarded_to_relay",
                }
                if state.active_turn is not None:
                    tts_started["turn_id"] = state.active_turn["turn_id"]
                state.trace.emit(
                    "harness",
                    "tts.started",
                    tts_started,
                    session_id=state.session_id,
                )
                state.observe_turn_event("tts.started", tts_started)
                state.trace.emit(
                    "harness.inferred",
                    "orb.state",
                    {"state": "speaking", "basis": "app.speak", "ui_confirmed": False},
                    session_id=state.session_id,
                )
        else:
            state.trace.emit("app", "app.invalid_control", {"text": str(message)[:500]}, session_id=state.session_id)
        await state.send_backend(message)


async def _backend_to_app(state: ProxyState, backend: Any, app: ServerConnection) -> None:
    async for message in backend:
        if isinstance(message, bytes):
            event = "tts.first_audio" if not state.tts_audio_seen else "tts.audio_chunk"
            state.tts_audio_seen = True
            trace_payload: dict[str, Any] = {
                "bytes": len(message),
                "sha256": hashlib.sha256(message).hexdigest(),
                "tenant_id": state.tenant_id,
                "session_id": state.session_id,
                "provider_streaming": False,
                "transport_streaming": True,
                "timing_scope": "first_audio_chunk_received_from_relay",
            }
            if event == "tts.first_audio" and state.active_turn is not None:
                trace_payload["turn_id"] = state.active_turn["turn_id"]
                trace_payload["inject_to_first_audio_ms"] = round(
                    (time.monotonic() - float(state.active_turn["started_monotonic"])) * 1000.0,
                    3,
                )
            state.trace.emit(
                "relay-rs",
                event,
                trace_payload,
                session_id=state.session_id,
            )
            state.observe_turn_event(event, trace_payload)
            state.append_tts_audio(message)
        else:
            payload = _json(message)
            if payload:
                state.record_json("relay-rs", payload)
            else:
                state.trace.emit("relay-rs", "invalid_control", {"text": str(message)[:500]}, session_id=state.session_id)
        await app.send(message)


async def app_handler(state: ProxyState, app: ServerConnection) -> None:
    async with state.lock:
        if state.app is not None:
            await state.app.close(code=1012, reason="replaced by newer test session")
        state.clear_app_identity()
        state.tts_audio_seen = False
        state.app = app
        state.backend = await websockets.connect(state.backend_url, max_size=None)
        state.connection_generation += 1
    state.trace.emit("harness", "connection.accepted", {"backend_url": state.backend_url})
    backend = state.backend
    try:
        await asyncio.gather(_app_to_backend(state, app), _backend_to_app(state, backend, app))
    except websockets.ConnectionClosed:
        pass
    except Exception as exc:
        state.trace.emit("harness", "proxy.error", {"error": str(exc)}, session_id=state.session_id)
    finally:
        async with state.lock:
            if state.backend is backend:
                await backend.close()
                state.backend = None
            if state.app is app:
                state.app = None
        state.trace.emit("harness", "connection.closed", {}, session_id=state.session_id)


async def control_handler(state: ProxyState, control: ServerConnection) -> None:
    injection_identity: tuple[str, str] | None = None
    injection_generation: int | None = None
    await control.send(json.dumps({"type": "ready", "connected": state.connected}))
    async for message in control:
        if isinstance(message, bytes):
            try:
                live_identity = state.current_identity()
                if injection_identity is None or injection_generation is None:
                    raise RuntimeError("INJECTION_NOT_STARTED: send start_listening before PCM frames")
                if live_identity != injection_identity or state.identity_generation != injection_generation:
                    raise RuntimeError(
                        "APP_IDENTITY_CHANGED: app session identity changed during PCM injection; "
                        "audio was not mixed across sessions"
                    )
                await state.send_backend(message)
                state.trace.emit(
                    "harness",
                    "mic.injected",
                    {"bytes": len(message), "sha256": hashlib.sha256(message).hexdigest()},
                    session_id=state.session_id,
                )
                await control.send(json.dumps({"type": "ack", "kind": "audio", "bytes": len(message)}))
            except Exception as exc:
                state.trace.emit("harness", "mic.inject_failed", {"error": str(exc)}, session_id=state.session_id)
                await control.send(json.dumps({"type": "error", "error": str(exc)}))
            continue
        payload = _json(message) or {}
        command = str(payload.get("type", ""))
        try:
            if command == "status":
                response = {
                    "type": "status",
                    "connected": state.connected,
                    "session_id": state.session_id,
                    "tenant_id": state.tenant_id,
                    "connection_generation": state.connection_generation,
                    "identity_generation": state.identity_generation,
                }
            elif command == "start_listening":
                frame, live_identity = state.injected_frame(payload)
                await state.send_backend(json.dumps(frame))
                injection_identity = live_identity
                injection_generation = state.identity_generation
                state.trace.emit(
                    "harness",
                    "inject.start_listening",
                    {
                        **frame,
                        "identity_source": (
                            "app_observed"
                            if (frame["tenant_id"], frame["session_id"]) == live_identity
                            else "negative_control_override"
                        ),
                    },
                    session_id=live_identity[1],
                )
                response = {
                    "type": "injection_started",
                    "kind": command,
                    "accepted": False,
                    "tenant_id": live_identity[0],
                    "session_id": live_identity[1],
                }
            elif command == "end_of_turn":
                live_identity = state.current_identity()
                if injection_identity != live_identity or injection_generation != state.identity_generation:
                    raise RuntimeError(
                        "APP_IDENTITY_CHANGED: no valid start_listening lease for the current app session"
                    )
                frame, live_identity = state.injected_frame(payload)
                turn = state.begin_turn(frame, live_identity)
                await state.send_backend(json.dumps(frame))
                response = {
                    "type": "injection_started",
                    "kind": command,
                    "accepted": False,
                    "turn_id": turn["turn_id"],
                    "tenant_id": live_identity[0],
                    "session_id": live_identity[1],
                }
            elif command == "await_turn":
                response = await state.await_turn(str(payload.get("turn_id", "")))
                injection_identity = None
                injection_generation = None
            elif command == "await_turn_stage":
                response = await state.await_turn_stage(
                    str(payload.get("turn_id", "")),
                    str(payload.get("stage", "")),
                )
            elif command in {"speak", "barge_in", "pause"}:
                frame, live_identity = state.injected_frame(payload)
                await state.send_backend(json.dumps(frame))
                state.trace.emit(
                    "harness",
                    f"inject.{command}",
                    frame,
                    session_id=live_identity[1],
                )
                response = {"type": "ack", "kind": command, "accepted": False}
            elif command == "close":
                if state.app is not None:
                    await state.app.close(code=1000, reason="test requested close")
                response = {"type": "ack", "kind": "close"}
            elif command == "inject_failure":
                reason = str(payload.get("reason", "provider_failure"))
                kind = str(payload.get("kind", "provider"))
                failure = {"type": "closing", "reason": reason, "kind": kind}
                state.trace.emit("harness", "failure", failure, session_id=state.session_id)
                if state.app is None:
                    raise RuntimeError("no app session is connected")
                # Send the same protocol-level closing frame the relay emits. This
                # keeps the fault inside the app's real receive path; it is not a
                # backend-only mock and does not require changing product code.
                await state.app.send(json.dumps(failure))
                response = {"type": "ack", "kind": "inject_failure", "reason": reason}
            elif command == "snapshot":
                evidence = await asyncio.to_thread(
                    state.snapshot,
                    str(payload.get("label", "checkpoint")),
                    str(payload.get("observation", "")),
                )
                response = {"type": "ack", "kind": "snapshot", **evidence}
            elif command == "configure_simulator":
                state.simulator_udid = str(payload.get("simulator_udid", "")) or None
                screenshots_dir = str(payload.get("screenshots_dir", ""))
                state.screenshots_dir = Path(screenshots_dir) if screenshots_dir else None
                state.app_bundle_id = str(payload.get("bundle_id", "")) or None
                state.trace.emit("harness", "simulator.configured", {"simulator_udid": state.simulator_udid, "screenshots_dir": screenshots_dir})
                response = {"type": "ack", "kind": "configure_simulator"}
            elif command == "relaunch":
                if not state.simulator_udid or not state.app_bundle_id:
                    raise RuntimeError("simulator is not configured for relaunch")
                completed = await asyncio.to_thread(
                    subprocess.run,
                    ["xcrun", "simctl", "launch", state.simulator_udid, state.app_bundle_id],
                    capture_output=True,
                    text=True,
                    check=False,
                )
                if completed.returncode != 0:
                    raise RuntimeError(completed.stderr.strip() or "simctl relaunch failed")
                state.trace.emit("simulator", "app.relaunched", {"bundle_id": state.app_bundle_id, "stdout": completed.stdout.strip()})
                response = {"type": "ack", "kind": "relaunch"}
            elif command == "fresh_launch":
                if not state.simulator_udid or not state.app_bundle_id:
                    raise RuntimeError("simulator is not configured for fresh launch")
                await asyncio.to_thread(
                    subprocess.run,
                    ["xcrun", "simctl", "terminate", state.simulator_udid, state.app_bundle_id],
                    capture_output=True,
                    text=True,
                    check=False,
                )
                completed = await asyncio.to_thread(
                    subprocess.run,
                    ["xcrun", "simctl", "launch", state.simulator_udid, state.app_bundle_id],
                    capture_output=True,
                    text=True,
                    check=False,
                )
                if completed.returncode != 0:
                    raise RuntimeError(completed.stderr.strip() or "simctl fresh launch failed")
                state.trace.emit(
                    "simulator",
                    "app.fresh_launch",
                    {"bundle_id": state.app_bundle_id, "stdout": completed.stdout.strip()},
                )
                response = {"type": "ack", "kind": "fresh_launch"}
            elif command == "journey_marker":
                journey_id = str(payload.get("journey_id", ""))
                phase = str(payload.get("phase", "marker"))
                state.trace.emit(
                    "harness",
                    f"journey.{phase}",
                    {
                        "journey_id": journey_id,
                        "description": str(payload.get("description", "")),
                        "expectation": payload.get("expectation"),
                        "error": payload.get("error"),
                    },
                    session_id=state.session_id,
                )
                response = {"type": "ack", "kind": "journey_marker", "journey_id": journey_id, "phase": phase}
            else:
                response = {"type": "error", "error": f"unknown control command: {command}"}
            await control.send(json.dumps(response))
        except Exception as exc:
            if command == "end_of_turn":
                state.active_turn = None
            state.trace.emit("harness", "control.error", {"command": command, "error": str(exc)}, session_id=state.session_id)
            await control.send(json.dumps({"type": "error", "error": str(exc)}))


def _install_shutdown_handlers(loop: asyncio.AbstractEventLoop, stop: asyncio.Event) -> None:
    # SIGINT already unwinds through the running coroutine's try/finally (Python
    # converts it to a KeyboardInterrupt at the next bytecode boundary). SIGTERM does
    # not: the process would exit immediately, skipping trace.emit/trace.close and
    # abandoning any in-flight subprocess (screenshot/OCR/simctl) launched via
    # asyncio.to_thread. `_terminate()` in run_e2e.py sends SIGTERM to this process,
    # so both signals must resolve `stop` and let `serve()`'s own finally run.
    for sig in (signal.SIGTERM, signal.SIGINT):
        loop.add_signal_handler(sig, stop.set)


async def serve(args: argparse.Namespace) -> None:
    trace = TraceWriter(Path(args.trace), args.run_id)
    state = ProxyState(
        args.backend_url,
        trace,
        suppress_app_audio=not args.forward_app_audio,
        simulator_udid=args.simulator_udid,
        screenshots_dir=Path(args.screenshots_dir) if args.screenshots_dir else None,
        tts_audio_dir=Path(args.tts_audio_dir) if args.tts_audio_dir else None,
        ocr_script=Path(args.ocr_script) if args.ocr_script else None,
        turn_timeout_seconds=args.turn_timeout_seconds,
        allow_identity_mismatch_test=args.allow_identity_mismatch_test,
    )
    trace.emit("harness", "proxy.started", {"app_port": args.app_port, "control_port": args.control_port, "backend_url": args.backend_url})
    trace.emit("harness.inferred", "orb.state", {"state": "booting", "basis": "proxy.started", "ui_confirmed": False})
    if args.expected_transcript:
        trace.emit(
            "harness",
            "run.expectations",
            {"expected_transcripts": [{"text": text, "max_word_error_rate": args.max_word_error_rate} for text in args.expected_transcript]},
        )
    async with websockets.serve(lambda ws: app_handler(state, ws), "127.0.0.1", args.app_port, max_size=None), websockets.serve(
        lambda ws: control_handler(state, ws), "127.0.0.1", args.control_port, max_size=None
    ):
        print(json.dumps({"app_url": f"ws://127.0.0.1:{args.app_port}", "control_url": f"ws://127.0.0.1:{args.control_port}", "trace": str(args.trace)}), flush=True)
        stop = asyncio.Event()
        _install_shutdown_handlers(asyncio.get_running_loop(), stop)
        try:
            await stop.wait()
        finally:
            trace.emit("harness", "proxy.stopped", {})
            trace.close()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app-port", type=int, default=8091)
    parser.add_argument("--control-port", type=int, default=8093)
    parser.add_argument("--backend-url", default="ws://127.0.0.1:8092")
    parser.add_argument("--trace", type=Path, required=True)
    parser.add_argument("--run-id", default="live-e2e")
    parser.add_argument("--forward-app-audio", action="store_true")
    parser.add_argument("--expected-transcript", action="append", default=[], help="Expected final transcript; repeat per journey turn")
    parser.add_argument("--max-word-error-rate", type=float, default=0.35)
    parser.add_argument("--simulator-udid", help="Simulator to screenshot for visible checkpoints")
    parser.add_argument("--screenshots-dir", help="Directory for checkpoint screenshots")
    parser.add_argument("--tts-audio-dir", help="Directory for captured relay TTS WAV files")
    parser.add_argument("--ocr-script", help="Vision OCR script used for every checkpoint")
    parser.add_argument("--turn-timeout-seconds", type=float, default=30.0)
    parser.add_argument(
        "--allow-identity-mismatch-test",
        action="store_true",
        help="Enable the explicit negative-control identity override; never use for a passing run",
    )
    args = parser.parse_args()
    try:
        asyncio.run(serve(args))
    except KeyboardInterrupt:
        return 130
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
