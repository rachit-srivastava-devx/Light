from __future__ import annotations

from io import StringIO
import json
from pathlib import Path
import struct
import tempfile
import unittest
import wave

from replay.audio import FrameSpec, WavValidationError, load_pcm16_mono_16k, split_pcm_frames
from replay.events import JsonlEventWriter
from replay.engine import MicrophoneReplayer
from replay.timing import ReplayClock


def write_wav(path: Path, *, samples: list[int], rate: int = 16_000, channels: int = 1, width: int = 2) -> None:
    with wave.open(str(path), "wb") as output:
        output.setnchannels(channels)
        output.setsampwidth(width)
        output.setframerate(rate)
        payload = b"".join(struct.pack("<h", sample) for sample in samples)
        output.writeframes(payload)


class RecordingTransport:
    def __init__(self) -> None:
        self.controls: list[dict[str, object]] = []
        self.audio: list[bytes] = []

    def send_control(self, frame: dict[str, object]) -> None:
        self.controls.append(dict(frame))

    def send_audio(self, payload: bytes) -> None:
        self.audio.append(payload)


class ReplayTests(unittest.TestCase):
    def test_validates_required_pcm16_mono_16k_format(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "valid.wav"
            write_wav(path, samples=[1] * 321)
            audio = load_pcm16_mono_16k(path)
            self.assertEqual(audio.sample_rate_hz, 16_000)
            self.assertEqual(audio.sample_count, 321)
            self.assertAlmostEqual(audio.duration_ms, 20.0625)

    def test_rejects_non_mono_or_non_16k_inputs(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            stereo = Path(directory) / "stereo.wav"
            write_wav(stereo, samples=[1, 2] * 10, channels=2)
            with self.assertRaises(WavValidationError):
                load_pcm16_mono_16k(stereo)

            rate = Path(directory) / "rate.wav"
            write_wav(rate, samples=[1] * 10, rate=8_000)
            with self.assertRaises(WavValidationError):
                load_pcm16_mono_16k(rate)

    def test_splits_20ms_frames_and_retains_partial_tail(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "tail.wav"
            write_wav(path, samples=list(range(323)))
            frames = split_pcm_frames(load_pcm16_mono_16k(path), FrameSpec())
            self.assertEqual([frame.source_sample_count for frame in frames], [320, 3])
            self.assertEqual([len(frame.pcm) for frame in frames], [640, 6])
            self.assertEqual(frames[1].sequence, 1)
            self.assertAlmostEqual(frames[1].stream_start_ms, 20.0)
            self.assertAlmostEqual(frames[1].stream_duration_ms, 0.1875)

    def test_replay_emits_exact_relay_sequence_and_jsonl_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "hello.wav"
            write_wav(path, samples=[7] * 323)
            transport = RecordingTransport()
            output = StringIO()
            replayer = MicrophoneReplayer(
                transport,
                tenant_id="t0",
                session_id="s1",
                frame_spec=FrameSpec(),
                clock=ReplayClock(speed=8.0, sleep_fn=lambda _: None),
                events=JsonlEventWriter(output),
            )
            result = replayer.replay(path, utterance_id="hello")

            self.assertEqual(
                transport.controls,
                [
                    {"type": "start_listening", "tenant_id": "t0", "session_id": "s1"},
                    {"type": "end_of_turn", "tenant_id": "t0", "session_id": "s1"},
                ],
            )
            self.assertEqual([len(payload) for payload in transport.audio], [640, 6])
            self.assertEqual(result.frames_sent, 2)
            events = [json.loads(line) for line in output.getvalue().splitlines()]
            self.assertEqual(
                [event["event"] for event in events],
                [
                    "replay.started",
                    "control.sent",
                    "audio.frame.sent",
                    "audio.frame.sent",
                    "control.sent",
                    "replay.completed",
                ],
            )
            self.assertEqual(events[2]["bytes"], 640)
            self.assertEqual(events[0]["event_index"], 0)
            self.assertEqual(events[2]["stream_start_ms"], 0.0)
            self.assertEqual(events[3]["stream_start_ms"], 20.0)
            self.assertEqual(events[-1]["transmitted_duration_ms"], 20.1875)
            self.assertNotIn("pcm", output.getvalue())


if __name__ == "__main__":
    unittest.main()
