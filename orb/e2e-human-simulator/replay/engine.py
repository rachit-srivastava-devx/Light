"""Transport-neutral deterministic microphone replay."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Mapping, Protocol

from .audio import FrameSpec, load_pcm16_mono_16k, split_pcm_frames
from .events import EventWriter, MemoryEventWriter
from .timing import ReplayClock


class RelaySink(Protocol):
    """Adapter boundary for a WebSocket or another relay-compatible transport."""

    def send_control(self, payload: Mapping[str, object]) -> None:
        """Send a JSON control frame."""

    def send_audio(self, pcm: bytes) -> None:
        """Send one binary raw PCM frame."""


@dataclass(frozen=True)
class ReplayResult:
    session_id: str
    fixture: str
    frames_sent: int
    bytes_sent: int
    source_duration_ms: float
    transmitted_duration_ms: float


class MicrophoneReplayer:
    """Replay one validated WAV as relay-compatible microphone frames."""

    def __init__(
        self,
        sink: RelaySink,
        *,
        tenant_id: str = "t0",
        session_id: str | None = None,
        frame_spec: FrameSpec = FrameSpec(),
        clock: ReplayClock | None = None,
        events: EventWriter | None = None,
    ) -> None:
        if not tenant_id:
            raise ValueError("tenant_id must not be empty")
        self.sink = sink
        self.tenant_id = tenant_id
        # Stable by default; callers can override this for parallel runs.
        self.session_id = session_id or "replay-session"
        self.frame_spec = frame_spec
        self.clock = clock or ReplayClock()
        self.events = events or MemoryEventWriter()

    def replay(self, wav_path: str | Path, *, utterance_id: str | None = None) -> ReplayResult:
        path = Path(wav_path)
        fixture = utterance_id or path.stem
        try:
            audio = load_pcm16_mono_16k(path)
            frames = split_pcm_frames(audio, self.frame_spec)
            self.events.emit(
                "replay.started",
                tenant_id=self.tenant_id,
                session_id=self.session_id,
                fixture=fixture,
                sample_rate_hz=audio.sample_rate_hz,
                channels=audio.channels,
                sample_width_bytes=audio.sample_width_bytes,
                source_duration_ms=audio.duration_ms,
                frame_duration_ms=self.frame_spec.frame_duration_ms,
                speed=self.clock.speed,
                at_ms=0.0,
            )

            start_payload = {
                "type": "start_listening",
                "tenant_id": self.tenant_id,
                "session_id": self.session_id,
            }
            self.sink.send_control(start_payload)
            self.events.emit("control.sent", direction="client_to_relay", payload=start_payload, at_ms=0.0)

            transmitted_duration_ms = 0.0
            bytes_sent = 0
            for frame_index, frame in enumerate(frames):
                if frame_index:
                    self.clock.wait_for_frame(frames[frame_index - 1].stream_duration_ms)
                self.sink.send_audio(frame.pcm)
                bytes_sent += len(frame.pcm)
                transmitted_duration_ms = frame.stream_start_ms + frame.stream_duration_ms
                self.events.emit(
                    "audio.frame.sent",
                    direction="client_to_relay",
                    session_id=self.session_id,
                    frame_seq=frame.sequence,
                    bytes=len(frame.pcm),
                    source_sample_offset=frame.source_sample_offset,
                    source_sample_count=frame.source_sample_count,
                    stream_start_ms=frame.stream_start_ms,
                    stream_duration_ms=frame.stream_duration_ms,
                    padded=frame.padded,
                    at_ms=frame.stream_start_ms,
                )

            if frames:
                self.clock.wait_for_frame(frames[-1].stream_duration_ms)

            end_payload = {
                "type": "end_of_turn",
                "tenant_id": self.tenant_id,
                "session_id": self.session_id,
            }
            self.sink.send_control(end_payload)
            self.events.emit(
                "control.sent",
                direction="client_to_relay",
                payload=end_payload,
                at_ms=audio.duration_ms,
            )
            result = ReplayResult(
                session_id=self.session_id,
                fixture=fixture,
                frames_sent=len(frames),
                bytes_sent=bytes_sent,
                source_duration_ms=audio.duration_ms,
                transmitted_duration_ms=transmitted_duration_ms,
            )
            self.events.emit(
                "replay.completed",
                tenant_id=self.tenant_id,
                session_id=self.session_id,
                fixture=fixture,
                frames_sent=result.frames_sent,
                bytes_sent=result.bytes_sent,
                source_duration_ms=result.source_duration_ms,
                transmitted_duration_ms=result.transmitted_duration_ms,
                at_ms=audio.duration_ms,
            )
            return result
        except Exception as exc:
            self.events.emit(
                "replay.failed",
                tenant_id=self.tenant_id,
                session_id=self.session_id,
                fixture=fixture,
                error_type=type(exc).__name__,
                error=str(exc),
            )
            raise
