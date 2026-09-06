"""Independent deterministic microphone replay primitives."""

from .audio import AudioFrame, FrameSpec, PcmAudio, WavValidationError, load_pcm16_mono_16k, split_pcm_frames
from .events import EventWriter, JsonlEventWriter, MemoryEventWriter
from .fixtures import ScriptedUtterance, load_fixture, load_fixture_manifest
from .engine import MicrophoneReplayer, RelaySink, ReplayResult
from .timing import ReplayClock

__all__ = [
    "AudioFrame",
    "EventWriter",
    "FrameSpec",
    "JsonlEventWriter",
    "MemoryEventWriter",
    "MicrophoneReplayer",
    "PcmAudio",
    "RelaySink",
    "ReplayResult",
    "ReplayClock",
    "ScriptedUtterance",
    "WavValidationError",
    "load_fixture",
    "load_fixture_manifest",
    "load_pcm16_mono_16k",
    "split_pcm_frames",
]
