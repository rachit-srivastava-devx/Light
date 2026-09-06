import json
import tempfile
import unittest
from pathlib import Path

from evaluator.trace_schema import load_jsonl


ROOT = Path(__file__).parents[1]


class TraceSchemaTests(unittest.TestCase):
    def test_fixture_loads_and_orders_by_timestamp(self):
        trace = load_jsonl(ROOT / "fixtures" / "good.jsonl")
        self.assertEqual(len(trace.schema_errors), 0)
        self.assertEqual(trace.ordered_events[0].event, "run.expectations")
        self.assertEqual(trace.ordered_events[-1].data["state"], "idle")

    def test_malformed_lines_are_evidence_not_crashes(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "bad.jsonl"
            path.write_text("not-json\n[]\n{\"seq\": 1}\n", encoding="utf-8")
            trace = load_jsonl(path)
        self.assertEqual(trace.events, [])
        self.assertEqual(len(trace.schema_errors), 3)

    def test_judge_evidence_redacts_sensitive_values(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "redact.jsonl"
            path.write_text(json.dumps({"ts": 1, "seq": 1, "source": "test", "event": "x", "data": {"api_key": "secret", "text": "hello"}}) + "\n", encoding="utf-8")
            trace = load_jsonl(path)
        self.assertEqual(trace.evidence()["events"][0]["data"]["api_key"], "[REDACTED]")


if __name__ == "__main__":
    unittest.main()
