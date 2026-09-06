import unittest
from pathlib import Path

from evaluator.checks import evaluate
from evaluator.judges import JudgeResult
from evaluator.report import render_report
from evaluator.trace_schema import load_jsonl


ROOT = Path(__file__).parents[1]


class ReportTests(unittest.TestCase):
    def test_report_contains_user_facing_sections_and_evidence(self):
        evaluation = evaluate(load_jsonl(ROOT / "fixtures" / "problematic.jsonl"))
        report = render_report(evaluation, [JudgeResult("codex", "unavailable", "codex missing")])
        for section in ("What felt wrong", "Evidence", "Severity and suggestions", "Optional judge review"):
            self.assertIn(f"## {section}", report)
        self.assertIn("STT_WORD_ERROR_RATE", report)
        self.assertIn("codex missing", report)


if __name__ == "__main__":
    unittest.main()
