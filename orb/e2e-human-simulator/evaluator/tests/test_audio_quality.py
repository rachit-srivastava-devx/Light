import unittest
from pathlib import Path

from evaluator.audio_quality import (
    AudioGateConfig,
    AudioQualityError,
    TranscriptionResult,
    compute_jiwer_metrics,
    run_audio_quality_gate,
)


class AudioQualityGateTests(unittest.TestCase):
    def test_all_layers_pass_with_good_evidence(self):
        report = run_audio_quality_gate(
            stt_audio=Path("mic.wav"),
            expected_transcript="hello how are you",
            tts_audio=Path("tts.wav"),
            tts_reference_audio=Path("tts-clean.wav"),
            nisqa_model=Path("nisqa_tts.tar"),
            transcriber=lambda _path, _config: TranscriptionResult("hello how are you", "en", 0.99, 1),
            jiwer_runner=lambda _reference, _hypothesis: {"wer": 0.0, "cer": 0.0, "mer": 0.0, "wil": 0.0},
            stoi_runner=lambda _reference, _candidate: {"stoi": 0.91},
            nisqa_runner=lambda _audio, _model, _device: {"mos": 4.1, "mos_pred": 4.1},
        )
        self.assertTrue(report.passed)
        self.assertEqual([layer.status for layer in report.layers], ["pass", "pass", "pass"])

    def test_bad_stt_and_audio_quality_are_reported_together(self):
        report = run_audio_quality_gate(
            stt_audio=Path("mic.wav"),
            expected_transcript="hello how are you",
            tts_audio=Path("tts.wav"),
            tts_reference_audio=Path("tts-clean.wav"),
            nisqa_model=Path("nisqa_tts.tar"),
            transcriber=lambda _path, _config: TranscriptionResult("random hindi", "hi", 0.88, 1),
            jiwer_runner=lambda _reference, _hypothesis: {"wer": 0.75, "cer": 0.60, "mer": 0.75, "wil": 0.8},
            stoi_runner=lambda _reference, _candidate: {"stoi": 0.20},
            nisqa_runner=lambda _audio, _model, _device: {"mos": 1.8},
        )
        self.assertFalse(report.passed)
        self.assertEqual([layer.status for layer in report.layers], ["fail", "fail", "fail"])
        self.assertEqual(
            {layer.error_code for layer in report.layers},
            {"STT_ACCURACY_BELOW_THRESHOLD", "TTS_INTELLIGIBILITY_BELOW_THRESHOLD", "TTS_NATURALNESS_BELOW_THRESHOLD"},
        )

    def test_missing_dependency_is_blocked_not_passed(self):
        def missing(_path, _config):
            raise AudioQualityError("AUDIO_DEPENDENCY_MISSING", "faster-whisper missing")

        report = run_audio_quality_gate(
            stt_audio=Path("mic.wav"),
            expected_transcript="hello",
            tts_audio=None,
            tts_reference_audio=None,
            nisqa_model=None,
            transcriber=missing,
            stoi_runner=lambda _reference, _candidate: {"stoi": 1.0},
            nisqa_runner=lambda _audio, _model, _device: {"mos": 5.0},
        )
        self.assertFalse(report.passed)
        self.assertEqual(report.layers[0].status, "blocked")
        self.assertEqual(report.layers[0].error_code, "AUDIO_DEPENDENCY_MISSING")
        self.assertTrue(all(layer.status == "blocked" for layer in report.layers[1:]))

    def test_jiwer_is_lazy_and_missing_dependency_is_actionable(self):
        try:
            metrics = compute_jiwer_metrics("hello", "hello")
        except AudioQualityError as error:
            self.assertEqual(error.code, "AUDIO_DEPENDENCY_MISSING")
        else:
            self.assertEqual(metrics["wer"], 0.0)

    def test_custom_thresholds_are_recorded(self):
        config = AudioGateConfig(max_wer=0.1, min_stoi=0.9, min_nisqa_mos=4.0)
        report = run_audio_quality_gate(
            stt_audio=Path("mic.wav"),
            expected_transcript="hello",
            tts_audio=Path("tts.wav"),
            tts_reference_audio=Path("clean.wav"),
            nisqa_model=Path("nisqa.tar"),
            config=config,
            transcriber=lambda _path, _config: TranscriptionResult("hello"),
            jiwer_runner=lambda _reference, _hypothesis: {"wer": 0.0, "cer": 0.0},
            stoi_runner=lambda _reference, _candidate: {"stoi": 0.91},
            nisqa_runner=lambda _audio, _model, _device: {"mos": 4.0},
        )
        self.assertEqual(report.to_dict()["config"]["min_nisqa_mos"], 4.0)

    def test_tts_transcript_layer_catches_unintelligible_playback(self):
        calls = iter([TranscriptionResult("hello how are you"), TranscriptionResult("one")])
        report = run_audio_quality_gate(
            stt_audio=Path("mic.wav"),
            expected_transcript="hello how are you",
            tts_audio=Path("tts.wav"),
            tts_expected_transcript="Hey Rachit good to hear from you",
            tts_reference_audio=Path("tts-clean.wav"),
            nisqa_model=Path("nisqa_tts.tar"),
            transcriber=lambda _path, _config: next(calls),
            jiwer_runner=lambda reference, hypothesis: {
                "wer": 0.0 if hypothesis == "hello how are you" else 0.9,
                "cer": 0.0 if hypothesis == "hello how are you" else 0.8,
                "mer": 0.0,
                "wil": 0.0,
            },
            stoi_runner=lambda _reference, _candidate: {"stoi": 0.91},
            nisqa_runner=lambda _audio, _model, _device: {"mos": 4.1},
        )
        self.assertFalse(report.passed)
        tts_layer = next(layer for layer in report.layers if layer.name == "tts_transcript")
        self.assertEqual(tts_layer.status, "fail")
        self.assertEqual(tts_layer.error_code, "TTS_TRANSCRIPT_ACCURACY_BELOW_THRESHOLD")

    def test_pystoi_is_explicitly_not_run_without_human_reference(self):
        report = run_audio_quality_gate(
            stt_audio=Path("mic.wav"),
            expected_transcript="hello",
            tts_audio=Path("tts.wav"),
            tts_expected_transcript="hello",
            tts_reference_audio=None,
            nisqa_model=Path("nisqa_tts.tar"),
            config=AudioGateConfig(require_stoi_reference=False),
            transcriber=lambda _path, _config: TranscriptionResult("hello"),
            jiwer_runner=lambda _reference, _hypothesis: {"wer": 0.0, "cer": 0.0, "mer": 0.0, "wil": 0.0},
            nisqa_runner=lambda _audio, _model, _device: {"mos": 4.0},
        )
        self.assertTrue(report.passed)
        stoi_layer = next(layer for layer in report.layers if layer.name == "tts_intelligibility")
        self.assertEqual(stoi_layer.status, "not_run")
        self.assertFalse(stoi_layer.required)


if __name__ == "__main__":
    unittest.main()
