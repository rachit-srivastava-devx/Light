"""Strict PCM audio loading and microphone-sized frame splitting."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Literal
import wave


class WavValidationError(ValueError):
    """Raised when an input WAV is not the microphone contract."""


TailPolicy = Literal["keep", "pad", "drop"]


@dataclass(frozen=True)
class PcmAudio:
    """Validated, headerless PCM16 mono audio."""

    pcm: bytes
    sample_rate_hz: int
    channels: int
    sample_width_bytes: int

    @property
    def sample_count(self) -> int:
        return len(self.pcm) // self.sample_width_bytes // self.channels

    @property
    def duration_ms(self) -> float:
        return self.sample_count * 1000 / self.sample_rate_hz


@dataclass(frozen=True)
class FrameSpec:
    """Frame shape used by a microphone transport."""

    sample_rate_hz: int = 16_000
    channels: int = 1
    sample_width_bytes: int = 2
    frame_duration_ms: int = 20
    tail_policy: TailPolicy = "keep"

    def __post_init__(self) -> None:
        if self.sample_rate_hz <= 0:
            raise ValueError("sample_rate_hz must be positive")
        if self.channels != 1:
            raise ValueError("the relay microphone contract requires mono audio")
        if self.sample_width_bytes != 2:
            raise ValueError("the relay microphone contract requires PCM16 audio")
        if self.frame_duration_ms <= 0:
            raise ValueError("frame_duration_ms must be positive")
        if (self.sample_rate_hz * self.frame_duration_ms) % 1000:
            raise ValueError("frame_duration_ms must produce a whole number of samples")
        if self.tail_policy not in {"keep", "pad", "drop"}:
            raise ValueError("tail_policy must be keep, pad, or drop")

    @property
    def samples_per_frame(self) -> int:
        return self.sample_rate_hz * self.frame_duration_ms // 1000

    @property
    def frame_bytes(self) -> int:
        return self.samples_per_frame * self.channels * self.sample_width_bytes


@dataclass(frozen=True)
class AudioFrame:
    """One binary microphone frame and its deterministic stream position."""

    sequence: int
    pcm: bytes
    source_sample_offset: int
    source_sample_count: int
    stream_start_ms: float
    stream_duration_ms: float
    padded: bool = False


def load_pcm16_mono_16k(path: str | Path) -> PcmAudio:
    """Load and strictly validate a PCM16 mono 16 kHz WAV file."""

    wav_path = Path(path)
    try:
        with wave.open(str(wav_path), "rb") as wav_file:
            channels = wav_file.getnchannels()
            sample_width_bytes = wav_file.getsampwidth()
            sample_rate_hz = wav_file.getframerate()
            compression = wav_file.getcomptype()
            frame_count = wav_file.getnframes()
            pcm = wav_file.readframes(frame_count)
    except (OSError, EOFError, wave.Error) as exc:
        raise WavValidationError(f"invalid WAV file {wav_path}: {exc}") from exc

    problems: list[str] = []
    if compression != "NONE":
        problems.append(f"compression must be NONE, got {compression!r}")
    if channels != 1:
        problems.append(f"channels must be 1, got {channels}")
    if sample_width_bytes != 2:
        problems.append(f"sample width must be 2 bytes, got {sample_width_bytes}")
    if sample_rate_hz != 16_000:
        problems.append(f"sample rate must be 16000 Hz, got {sample_rate_hz}")
    if frame_count <= 0:
        problems.append("audio must contain at least one sample")

    expected_bytes = frame_count * max(channels, 0) * max(sample_width_bytes, 0)
    if len(pcm) != expected_bytes:
        problems.append(f"payload length {len(pcm)} does not match WAV header {expected_bytes}")

    if problems:
        raise WavValidationError(f"{wav_path}: " + "; ".join(problems))

    return PcmAudio(
        pcm=pcm,
        sample_rate_hz=sample_rate_hz,
        channels=channels,
        sample_width_bytes=sample_width_bytes,
    )


def split_pcm_frames(audio: PcmAudio, spec: FrameSpec = FrameSpec()) -> tuple[AudioFrame, ...]:
    """Split validated PCM into deterministic frames without changing source samples.

    ``keep`` retains a short final frame, matching a stream that ends mid-frame.
    ``pad`` sends a full zero-padded final frame. ``drop`` omits the incomplete tail.
    """

    if (
        audio.sample_rate_hz != spec.sample_rate_hz
        or audio.channels != spec.channels
        or audio.sample_width_bytes != spec.sample_width_bytes
    ):
        raise WavValidationError("audio format does not match the requested frame specification")

    bytes_per_sample = spec.channels * spec.sample_width_bytes
    if len(audio.pcm) % bytes_per_sample:
        raise WavValidationError("PCM payload ends mid-sample")

    frames: list[AudioFrame] = []
    for sequence, start in enumerate(range(0, len(audio.pcm), spec.frame_bytes)):
        source_pcm = audio.pcm[start : start + spec.frame_bytes]
        source_sample_count = len(source_pcm) // bytes_per_sample
        is_tail = len(source_pcm) < spec.frame_bytes
        if is_tail and spec.tail_policy == "drop":
            break

        transmitted_pcm = source_pcm
        padded = False
        if is_tail and spec.tail_policy == "pad":
            transmitted_pcm += b"\x00" * (spec.frame_bytes - len(transmitted_pcm))
            padded = True

        transmitted_sample_count = len(transmitted_pcm) // bytes_per_sample
        frames.append(
            AudioFrame(
                sequence=sequence,
                pcm=transmitted_pcm,
                source_sample_offset=start // bytes_per_sample,
                source_sample_count=source_sample_count,
                stream_start_ms=(start // bytes_per_sample) * 1000 / spec.sample_rate_hz,
                stream_duration_ms=transmitted_sample_count * 1000 / spec.sample_rate_hz,
                padded=padded,
            )
        )
    return tuple(frames)
