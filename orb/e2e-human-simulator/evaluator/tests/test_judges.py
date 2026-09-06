import json
import os
import sys
import unittest
from pathlib import Path

from evaluator.checks import evaluate
from evaluator.judges import CommandJudgeAdapter, CodexJudgeAdapter, _parse_result, run_judges
from evaluator.trace_schema import load_jsonl


ROOT = Path(__file__).parents[1]


class JudgeAdapterTests(unittest.TestCase):
    def test_missing_cli_is_gracefully_reported(self):
        result = CommandJudgeAdapter("missing", "definitely-not-installed-orb-judge", ()).evaluate({"events": []})
        self.assertEqual(result.status, "unavailable")

    def test_json_output_is_parsed(self):
        result = _parse_result("codex", '{"score": 8, "summary": "clear", "findings": ["good latency"]}')
        self.assertEqual(result.status, "ok")
        self.assertEqual(result.score, 8)
        self.assertEqual(result.findings, ("good latency",))

    def test_command_adapter_only_receives_evidence_prompt(self):
        script = "import json,sys; prompt=sys.stdin.read(); print(json.dumps({'score': 9, 'summary': 'evidence only', 'findings': [str('CAPTURED_EVIDENCE' in prompt)]}))"
        adapter = CommandJudgeAdapter("fake", sys.executable, ("-c", script))
        result = adapter.evaluate({"events": [{"event": "assistant.response", "data": {"text": "hello"}}]})
        self.assertEqual(result.status, "ok")
        self.assertEqual(result.score, 9)
        self.assertEqual(result.findings, ("True",))

    def test_absolute_binary_path_is_supported(self):
        adapter = CommandJudgeAdapter("python-absolute", os.path.realpath(sys.executable), ("-c", "print('{\"score\": 7, \"summary\": \"ok\", \"findings\": []}')"))
        self.assertTrue(adapter.available())
        self.assertEqual(adapter.evaluate({"events": []}).status, "ok")

    def test_default_judges_do_not_make_missing_cli_a_failure(self):
        report = evaluate(load_jsonl(ROOT / "fixtures" / "good.jsonl"))
        results = run_judges(report, [CommandJudgeAdapter("missing", "definitely-not-installed-orb-judge", ())])
        self.assertEqual(results[0].status, "unavailable")


if __name__ == "__main__":
    unittest.main()
