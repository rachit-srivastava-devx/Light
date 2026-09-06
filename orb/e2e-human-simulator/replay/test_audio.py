from __future__ import annotations

from pathlib import Path
import tempfile
import unittest
import wave

from replay.audio import FrameSpec, WavValidationError, load_pcm16_mono_16k, split_pcm_frames


def write_wav(path: Path, *, samples: bytes, rate: int = 16_000, channels: int = 1, width: int = 2) -> None:
    with wave.open(str(path), "wb") as wav_file:
        wav_file.setnchannels(channels)
        wav_file.setsampwidth(width)
        wav_file.setframerate(rate)
        wav_file.writeframes(samples)


class AudioTests(unittest.TestCase):
    def test_loads_strict_microphone_contract(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "speech.wav"
            payload = bytes(range(32))
            write_wav(path, samples=payload)

            audio = load_pcm16_mono_16k(path)

        self.assertEqual(audio.pcm, payload)
        self.assertEqual(audio.sample_count, 16)
        self.assertEqual(audio.duration_ms, 1.0)

    def test_rejects_non_mono_wrong_rate_and_wrong_width(self) -> None:
        cases = ((2, 16_000, 2), (1, 8_000, 2), (1, 16_000, 1))
        with tempfile.TemporaryDirectory() as directory:
            for index, (channels, rate, width) in enumerate(cases):
                path = Path(directory) / f"bad-{index}.wav"
                payload = b"\x00" * (channels * width * 16)
                write_wav(path, samples=payload, channels=channels, rate=rate, width=width)
                with self.subTest(path=path):
                    with self.assertRaises(WavValidationError):
                        load_pcm16_mono_16k(path)

    def test_split_keep_pad_and_drop_tail_deterministically(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "tail.wav"
            payload = b"\x01\x02" * 641
            write_wav(path, samples=payload)
            audio = load_pcm16_mono_16k(path)

        keep = split_pcm_frames(audio, FrameSpec(tail_policy="keep"))
        pad = split_pcm_frames(audio, FrameSpec(tail_policy="pad"))
        drop = split_pcm_frames(audio, FrameSpec(tail_policy="drop"))

        # 20ms at 16kHz is 320 samples / 640 bytes. The 641-sample source therefore
        # produces two full frames and one one-sample tail.
        self.assertEqual([frame.source_sample_count for frame in keep], [320, 320, 1])
        self.assertEqual([len(frame.pcm) for frame in keep], [640, 640, 2])
        self.assertEqual([len(frame.pcm) for frame in pad], [640, 640, 640])
        self.assertTrue(pad[-1].padded)
        self.assertEqual(len(drop), 2)
        self.assertEqual(keep[-1].stream_start_ms, 40.0)

    def test_frame_spec_rejects_non_integer_frame_size(self) -> None:
        with self.assertRaises(ValueError):
            FrameSpec(frame_duration_ms=0)


if __name__ == "__main__":
    unittest.main()
