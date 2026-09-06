from __future__ import annotations

import json
import math
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))

from qscore import TBM, QReport, SubScoreReport, VetoState, naturalness_subscore


class NaturalnessSubscoreTests(unittest.TestCase):
    def test_missing_audio_is_tbm_not_pass(self) -> None:
        score = naturalness_subscore(None)

        self.assertEqual(score.value, TBM)
        self.assertIsNone(score.gate_passed)

    def test_utmos_json_becomes_normalized_measured_subscore(self) -> None:
        payload = {
            "instrument_version": "1.3.1.dev0",
            "samples": [
                {
                    "mos": 3.060772180557251,
                    "normalized_score": 0.5151930451393127,
                    "threshold_mos": 2.77,
                    "passed": True,
                }
            ],
        }
        with (
            tempfile.NamedTemporaryFile(suffix=".wav") as audio,
            patch("qscore.subprocess.run") as run,
        ):
            run.return_value.returncode = 0
            run.return_value.stdout = json.dumps(payload)
            run.return_value.stderr = ""
            score = naturalness_subscore(audio.name, utmos_python_path=sys.executable)

        self.assertEqual(score.value, 0.5152)
        self.assertTrue(score.gate_passed)
        self.assertIn("MOS=3.0608", score.instrument)

    def test_native_control_fails_the_calibrated_gate(self) -> None:
        payload = {
            "instrument_version": "1.3.1.dev0",
            "samples": [
                {
                    "mos": 2.652761459350586,
                    "normalized_score": 0.4131903648376465,
                    "threshold_mos": 2.77,
                    "passed": False,
                }
            ],
        }
        with (
            tempfile.NamedTemporaryFile(suffix=".wav") as audio,
            patch("qscore.subprocess.run") as run,
        ):
            run.return_value.returncode = 0
            run.return_value.stdout = json.dumps(payload)
            run.return_value.stderr = ""
            score = naturalness_subscore(audio.name, utmos_python_path=sys.executable)

        self.assertEqual(score.value, 0.4132)
        self.assertFalse(score.gate_passed)
        self.assertIn("margin=-0.1172", score.instrument)


class QAggregationTests(unittest.TestCase):
    def test_geometric_mean_uses_every_measured_dimension(self) -> None:
        values = [0.5152, 1.0, 0.75, 0.8, 1.0]
        report = QReport(
            veto=VetoState(),
            subscores=[
                SubScoreReport(str(index), value, "test", "test", gate_passed=True)
                for index, value in enumerate(values)
            ],
        )

        expected = math.prod(values) ** (1.0 / len(values))
        self.assertAlmostEqual(report.q, expected)
        self.assertEqual(report.status, "complete")

    def test_any_veto_forces_q_to_zero(self) -> None:
        report = QReport(
            veto=VetoState(native_tts_detected=True),
            subscores=[
                SubScoreReport("naturalness", 1.0, "test", "test", gate_passed=True)
            ],
        )

        self.assertEqual(report.q, 0.0)
        self.assertEqual(report.status, "vetoed")

    def test_measured_failure_is_not_hidden_by_tbm_dimension(self) -> None:
        report = QReport(
            veto=VetoState(),
            subscores=[
                SubScoreReport(
                    "naturalness", 0.4132, "UTMOSv2 native control", "test", gate_passed=False
                ),
                SubScoreReport("no_dead_air", TBM, "missing session audio", "test"),
            ],
        )

        self.assertEqual(report.status, "failed")


if __name__ == "__main__":
    unittest.main()
