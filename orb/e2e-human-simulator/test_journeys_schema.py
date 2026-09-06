"""E6 (Track E) — structural validation of `journeys.json`.

`journeys.json` is a DECLARATIVE spec — as of this writing no runner in this repo actually
executes it (grepped for `journeys.json` across `.py`/`.md`/`.json`/`.mjs`/`.ts`: zero hits outside
this file and the acceptance contract). `run_e2e.py`'s own `journeys` config key is a different,
simpler shape (`id`/`wav`/`speed`/`post_turn_wait`/`fault`, observational only) read from
`run_config*.json`, not from this file. So "keeping the existing 6 intact and passing" cannot mean
"a live simulator run stays green" today — nothing wires `journeys.json` to a live run yet. What
this test CAN make real: the file is valid JSON, every journey (all 9, not just the 3 new ones)
has a well-formed `steps` array alternating `action`/`expect` shapes the acceptance contract's own
vocabulary uses, ids are unique, and — surfaced rather than hidden — which referenced fixture WAV
files actually exist on disk right now. A missing fixture is reported as a named finding, not
silently passed.
"""

from __future__ import annotations

import json
import unittest
from pathlib import Path

_HARNESS_ROOT = Path(__file__).resolve().parent
_JOURNEYS_PATH = _HARNESS_ROOT / "journeys.json"
_FIXTURES_DIR = _HARNESS_ROOT / "replay" / "fixtures"

KNOWN_ACTIONS = {"fresh_launch", "speak_fixture", "barge_in", "inject_fault"}
# The vocabulary the existing 6 journeys already use, plus the additions this track introduced for
# the 3 new journeys (`mode`, `response_not_contains_any`) — documented in
# evals/TELEMETRY-CONTRACT.md and paired with the existing `response_contains_any` naming.
KNOWN_EXPECT_KEYS = {
    "speech_contains",
    "state",
    "transcript_contains",
    "response_not",
    "one_reply",
    "states_in_order",
    "backend_atomize_calls",
    "speech_non_empty",
    "progress_advanced",
    "response_contains_any",
    "no_backend_loop",
    "spoken_failure",
    "error_state",
    "failure_reply_count",
    "old_audio_cancelled",
    "mode",
    "response_not_contains_any",
}

# Journeys authored before this track's `mode` field existed (Track B/C's acceptance-contract B4
# addition) do not assert it — that is expected, not a defect in this file.
JOURNEYS_WITHOUT_MODE_ASSERTION = {
    "launch-greeting",
    "social-hello",
    "task-progress",
    "unclear-speech",
    "provider-failure",
    "barge-in",
}
NEW_JOURNEYS_TRACK_E_ADDED = {"teach-me-something", "open-domain-vent", "topic-switch"}


def load_journeys() -> dict:
    return json.loads(_JOURNEYS_PATH.read_text(encoding="utf-8"))


class JourneysSchemaTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.doc = load_journeys()
        cls.journeys = cls.doc["journeys"]
        cls.by_id = {j["id"]: j for j in cls.journeys}

    def test_document_has_version_and_at_least_nine_journeys(self) -> None:
        self.assertEqual(self.doc.get("version"), "focus-orb-human-journeys.v1")
        self.assertGreaterEqual(len(self.journeys), 9, "the 6 pre-existing + 3 new journeys must all be present")

    def test_the_six_pre_existing_journeys_are_intact(self) -> None:
        for expected_id in JOURNEYS_WITHOUT_MODE_ASSERTION:
            self.assertIn(expected_id, self.by_id, f"pre-existing journey {expected_id!r} is missing")

    def test_the_three_new_journeys_are_present(self) -> None:
        for expected_id in NEW_JOURNEYS_TRACK_E_ADDED:
            self.assertIn(expected_id, self.by_id, f"new journey {expected_id!r} is missing")

    def test_journey_ids_are_unique(self) -> None:
        ids = [j["id"] for j in self.journeys]
        self.assertEqual(len(ids), len(set(ids)), f"duplicate journey ids: {ids}")

    def test_every_journey_has_a_well_formed_steps_array(self) -> None:
        problems: list[str] = []
        for journey in self.journeys:
            journey_id = journey.get("id", "<no id>")
            if not isinstance(journey.get("description"), str) or not journey["description"].strip():
                problems.append(f"{journey_id}: missing/empty description")
            steps = journey.get("steps")
            if not isinstance(steps, list) or not steps:
                problems.append(f"{journey_id}: steps must be a non-empty array")
                continue
            has_expectation = False
            for index, step in enumerate(steps):
                if not isinstance(step, dict):
                    problems.append(f"{journey_id}[{index}]: step must be an object")
                    continue
                if "action" in step:
                    if step["action"] not in KNOWN_ACTIONS:
                        problems.append(f"{journey_id}[{index}]: unknown action {step['action']!r}")
                elif "expect" in step:
                    has_expectation = True
                    if not isinstance(step["expect"], dict) or not step["expect"]:
                        problems.append(f"{journey_id}[{index}]: expect must be a non-empty object")
                        continue
                    unknown_keys = set(step["expect"].keys()) - KNOWN_EXPECT_KEYS
                    if unknown_keys:
                        problems.append(f"{journey_id}[{index}]: unknown expect key(s) {sorted(unknown_keys)}")
                else:
                    problems.append(f"{journey_id}[{index}]: step has neither 'action' nor 'expect'")
            if not has_expectation:
                problems.append(f"{journey_id}: has no 'expect' step at all — nothing is actually asserted")
        self.assertEqual(problems, [], "journeys.json schema problems:\n" + "\n".join(problems))

    def test_new_journeys_assert_mode_where_the_field_is_expected_to_exist(self) -> None:
        for journey_id in NEW_JOURNEYS_TRACK_E_ADDED:
            journey = self.by_id[journey_id]
            expect_steps = [s["expect"] for s in journey["steps"] if "expect" in s]
            has_mode_assertion = any("mode" in expect for expect in expect_steps)
            if journey_id == "topic-switch":
                # topic-switch is deliberately mode-agnostic: the point of this journey is context
                # continuity across a subject change, which can legitimately stay in 'converse'
                # mode throughout — asserting a specific mode value here would test something this
                # journey does not claim to test.
                continue
            self.assertTrue(has_mode_assertion, f"{journey_id} should assert 'mode' per acceptance-contract B4")

    def test_fixture_files_referenced_are_reported_present_or_missing(self) -> None:
        """This does NOT fail the test suite on a missing fixture for the 6 pre-existing journeys
        (they predate this track and its fixture directory convention) — it reports, honestly,
        which ones exist. It DOES fail if any of the 3 journeys THIS track added reference a
        fixture that is not actually on disk, since those are this track's own claim.
        """
        missing_for_new_journeys: list[str] = []
        report_lines: list[str] = []
        for journey in self.journeys:
            for step in journey["steps"]:
                fixture = step.get("action") == "speak_fixture" and step.get("fixture")
                if not fixture:
                    continue
                exists = (_FIXTURES_DIR / fixture).is_file()
                report_lines.append(f"{journey['id']}: {fixture} -> {'present' if exists else 'MISSING'}")
                if not exists and journey["id"] in NEW_JOURNEYS_TRACK_E_ADDED:
                    missing_for_new_journeys.append(f"{journey['id']}: {fixture}")
        # Printed always so `-v` runs show the real inventory, not just a pass/fail bit.
        print("\nFixture inventory (" + str(_FIXTURES_DIR) + "):")
        for line in report_lines:
            print(f"  {line}")
        self.assertEqual(
            missing_for_new_journeys,
            [],
            "fixtures referenced by Track E's own new journeys are missing on disk: "
            f"{missing_for_new_journeys}",
        )


if __name__ == "__main__":
    unittest.main()
