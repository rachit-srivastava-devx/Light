import unittest
from pathlib import Path
import tempfile

from evaluator.checks import CheckConfig, evaluate, word_error_rate
from evaluator.trace_schema import load_jsonl


ROOT = Path(__file__).parents[1]


class DeterministicCheckTests(unittest.TestCase):
    def test_word_error_rate(self):
        self.assertEqual(word_error_rate("hello how are you", "hello how are you"), 0)
        self.assertGreater(word_error_rate("hello how are you", "नमस्ते"), 0.5)

    def test_good_fixture_passes(self):
        report = evaluate(load_jsonl(ROOT / "fixtures" / "good.jsonl"))
        self.assertTrue(report.passed, [finding.__dict__ for finding in report.findings])
        self.assertEqual(report.counts()["error"], 0)

    def test_problematic_fixture_exposes_the_user_visible_failures(self):
        report = evaluate(load_jsonl(ROOT / "fixtures" / "problematic.jsonl"))
        codes = {finding.code for finding in report.findings}
        self.assertFalse(report.passed)
        for expected in {"STT_WORD_ERROR_RATE", "LATENCY_GATEWAY", "RESPONSE_DUPLICATE", "RESPONSE_GENERIC_PRESENCE", "FAILURE_SPEECH_WRONG", "FISH_MARKUP_UNKNOWN_TAG", "FISH_MARKUP_NOT_STRIPPED"}:
            self.assertIn(expected, codes)

    def test_markup_is_required_but_short_response_can_be_warned(self):
        report = evaluate(load_jsonl(ROOT / "fixtures" / "good.jsonl"), CheckConfig(require_markup=True))
        self.assertNotIn("FISH_MARKUP_ABSENT", {finding.code for finding in report.findings})

    def test_visible_microphone_error_is_a_hard_ui_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            trace_path = Path(directory) / "trace.jsonl"
            trace_path.write_text(
                '{"ts":"2026-08-07T10:00:00Z","seq":1,"source":"simulator","event":"ui.screenshot","data":{"observation":"Mic error - check console"}}\n',
                encoding="utf-8",
            )
            report = evaluate(load_jsonl(trace_path))
            self.assertIn("UI_MIC_ERROR", {finding.code for finding in report.findings})
            self.assertFalse(report.passed)

    def test_known_simulator_mic_error_does_not_block_injected_pcm_lane(self):
        with tempfile.TemporaryDirectory() as directory:
            trace_path = Path(directory) / "trace.jsonl"
            trace_path.write_text(
                '{"ts":"2026-08-07T10:00:00Z","seq":1,"source":"simulator","event":"ui.screenshot","data":{"observation":"Mic error - check console","known_simulator_mic_error":true}}\n',
                encoding="utf-8",
            )
            report = evaluate(load_jsonl(trace_path))
            codes = {finding.code for finding in report.findings}
            self.assertIn("UI_MIC_ERROR_KNOWN_SIMULATOR", codes)
            self.assertNotIn("UI_MIC_ERROR", codes)


if __name__ == "__main__":
    unittest.main()
