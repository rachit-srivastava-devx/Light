"""Dry-run CLI that emits relay-compatible protocol evidence without third-party packages."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
from typing import Mapping

from .audio import FrameSpec
from .events import JsonlEventWriter
from .engine import MicrophoneReplayer
from .timing import ReplayClock


class EvidenceSink:
    """Transport adapter for offline inspection; it sends no network traffic."""

    def __init__(self, events: JsonlEventWriter) -> None:
        self.events = events

    def send_control(self, payload: Mapping[str, object]) -> None:
        self.events.emit("transport.control", payload=dict(payload))

    def send_audio(self, pcm: bytes) -> None:
        self.events.emit("transport.audio", bytes=len(pcm), sha256=hashlib.sha256(pcm).hexdigest())


def main() -> int:
    parser = argparse.ArgumentParser(description="Replay a PCM16 mono 16 kHz WAV as microphone frames")
    parser.add_argument("wav", type=Path)
    parser.add_argument("--events", type=Path, required=True, help="JSONL evidence output path")
    parser.add_argument("--speed", type=float, default=1.0, help="1.0 real-time; >1 accelerated")
    parser.add_argument("--frame-duration-ms", type=int, default=20)
    parser.add_argument("--tail-policy", choices=("keep", "pad", "drop"), default="keep")
    parser.add_argument("--tenant-id", default="t0")
    parser.add_argument("--session-id")
    args = parser.parse_args()

    with args.events.open("w", encoding="utf-8", newline="\n") as stream:
        events = JsonlEventWriter(stream)
        sink = EvidenceSink(events)
        replayer = MicrophoneReplayer(
            sink,
            tenant_id=args.tenant_id,
            session_id=args.session_id,
            frame_spec=FrameSpec(frame_duration_ms=args.frame_duration_ms, tail_policy=args.tail_policy),
            clock=ReplayClock(speed=args.speed),
            events=events,
        )
        result = replayer.replay(args.wav)
        events.emit("cli.result", **result.__dict__)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
