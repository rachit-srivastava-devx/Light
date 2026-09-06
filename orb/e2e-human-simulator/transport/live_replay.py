#!/usr/bin/env python3
"""Replay a WAV fixture through the proxy control socket."""

from __future__ import annotations

import argparse
import json
import time
from pathlib import Path

from websocket import ABNF, create_connection

from replay.audio import FrameSpec, load_pcm16_mono_16k, split_pcm_frames


def _recv_json(ws) -> dict:
    payload = json.loads(ws.recv())
    if payload.get("type") == "error":
        raise RuntimeError(str(payload.get("error", "proxy command failed")))
    return payload


def _snapshot(ws, label: str, observation: str, *, require_transition: bool) -> dict:
    ws.send(json.dumps({"type": "snapshot", "label": label, "observation": observation}))
    evidence = _recv_json(ws)
    if require_transition and evidence.get("ui_confirmed") is not True:
        raise RuntimeError(
            f"UI_TRANSITION_NOT_CONFIRMED: checkpoint {label!r} had no OCR/state-color evidence"
        )
    return evidence


def _await_stage(ws, turn_id: str, stage: str) -> None:
    ws.send(json.dumps({"type": "await_turn_stage", "turn_id": turn_id, "stage": stage}))
    observed = _recv_json(ws)
    if observed.get("type") != "turn_stage_observed" or observed.get("stage") != stage:
        raise RuntimeError(f"turn stage {stage!r} was not observed: {observed}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("fixture", type=Path)
    parser.add_argument("--control-url", default="ws://127.0.0.1:8093")
    parser.add_argument("--speed", type=float, default=1.0)
    parser.add_argument("--session-id", default="live-e2e", help="Deprecated; proxy always uses the observed app identity")
    parser.add_argument("--tenant-id", default="t0")
    parser.add_argument(
        "--force-session-id-for-test",
        help="Negative control only: requires proxy --allow-identity-mismatch-test and must fail red",
    )
    parser.add_argument("--snapshot-prefix", help="Capture listening/processing/complete checkpoints through the proxy")
    parser.add_argument("--snapshot-observation", default="", help="Human observation attached to each checkpoint")
    parser.add_argument("--post-turn-wait", type=float, default=2.0)
    args = parser.parse_args()
    if args.speed <= 0:
        raise SystemExit("--speed must be positive")
    ws = create_connection(args.control_url, timeout=10)
    try:
        ws.recv()
        ws.send(json.dumps({"type": "status"}))
        status = json.loads(ws.recv())
        if not status.get("connected"):
            raise SystemExit("proxy has no live app WebSocket; launch the app before replay")
        # The proxy, not this process, stamps each control frame with the identity most recently
        # observed from the app. This avoids latching an identity that can become stale after a
        # reconnect. The status check is only a fail-fast readiness gate.
        live_session_id = status.get("session_id")
        live_tenant_id = status.get("tenant_id")
        if not isinstance(live_session_id, str) or not live_session_id:
            raise SystemExit(
                "APP_IDENTITY_UNAVAILABLE: proxy has not observed the app session identity; "
                "refusing placeholder injection"
            )
        if not isinstance(live_tenant_id, str) or not live_tenant_id:
            raise SystemExit("APP_IDENTITY_UNAVAILABLE: proxy has not observed the app tenant identity")
        identity_override = (
            {"tenant_id": live_tenant_id, "session_id": args.force_session_id_for_test}
            if args.force_session_id_for_test
            else None
        )
        start_command = {"type": "start_listening"}
        if identity_override is not None:
            start_command["test_identity_override"] = identity_override
        ws.send(json.dumps(start_command))
        start_ack = _recv_json(ws)
        if start_ack.get("type") != "injection_started" or start_ack.get("accepted") is not False:
            raise RuntimeError(f"unexpected start_listening response: {start_ack}")
        screenshots: dict[str, dict] = {}
        if args.snapshot_prefix:
            screenshots["listening"] = _snapshot(
                ws,
                f"{args.snapshot_prefix}-listening",
                args.snapshot_observation,
                require_transition=True,
            )
        spec = FrameSpec(sample_rate_hz=16000, channels=1, sample_width_bytes=2, frame_duration_ms=20, tail_policy="keep")
        audio = load_pcm16_mono_16k(args.fixture)
        frames = split_pcm_frames(audio, spec)
        for frame in frames:
            ws.send(frame.pcm, opcode=ABNF.OPCODE_BINARY)
            _recv_json(ws)
            if args.speed != float("inf"):
                time.sleep((spec.frame_duration_ms / 1000) / args.speed)
        end_command = {"type": "end_of_turn"}
        if identity_override is not None:
            end_command["test_identity_override"] = identity_override
        ws.send(json.dumps(end_command))
        injection_started = _recv_json(ws)
        if injection_started.get("type") != "injection_started" or injection_started.get("accepted") is not False:
            raise RuntimeError(f"unexpected end_of_turn response: {injection_started}")
        turn_id = str(injection_started.get("turn_id", ""))
        if args.snapshot_prefix:
            _await_stage(ws, turn_id, "transcript")
            screenshots["processing"] = _snapshot(
                ws,
                f"{args.snapshot_prefix}-processing",
                args.snapshot_observation,
                require_transition=True,
            )
            _await_stage(ws, turn_id, "assistant_response")
            screenshots["speaking"] = _snapshot(
                ws,
                f"{args.snapshot_prefix}-speaking",
                args.snapshot_observation,
                require_transition=True,
            )
        ws.send(json.dumps({"type": "await_turn", "turn_id": turn_id}))
        acceptance = _recv_json(ws)
        if acceptance.get("type") != "turn_accepted":
            raise RuntimeError(f"relay did not prove injected-turn acceptance: {acceptance}")
        if args.snapshot_prefix:
            time.sleep(max(0.0, args.post_turn_wait))
            screenshots["complete"] = _snapshot(
                ws,
                f"{args.snapshot_prefix}-complete",
                args.snapshot_observation,
                require_transition=False,
            )
        print(
            json.dumps(
                {
                    "status": "accepted",
                    "fixture": str(args.fixture),
                    "acceptance": acceptance,
                    "screenshots": screenshots,
                }
            )
        )
    finally:
        ws.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
