import tempfile
import unittest
import json
from pathlib import Path
from unittest.mock import patch

from run_e2e import (
    _assert_app_healthy,
    _assert_spoken_recovery,
    _failure_status,
    _run_audio_gate,
    _turn_evidence,
)


class OrchestratorAudioGateTests(unittest.TestCase):
    def test_enabled_audio_gate_writes_log_and_propagates_exit(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            run_root = root / "run"
            run_root.mkdir()
            config = {
                "stt_audio": "{run_root}/input.wav",
                "expected_transcript": "hello",
                "tts_audio": "{run_root}/tts.wav",
                "tts_expected_transcript": "hello there",
                "tts_reference_audio": "clean.wav",
                "nisqa_model": "nisqa_tts.tar",
                "whisper_model": "small.en",
                "min_stoi": 0.8,
            }
            completed = type("Completed", (), {"returncode": 1, "stdout": "blocked", "stderr": "missing model"})()
            with patch("run_e2e.subprocess.run", return_value=completed) as run:
                result = _run_audio_gate(config, base=root, run_root=run_root, env={})

            self.assertEqual(result, 1)
            self.assertEqual((run_root / "audio-quality.log").read_text(encoding="utf-8"), "blockedmissing model")
            command = run.call_args.args[0]
            self.assertIn("evaluator.audio_quality", command)
            self.assertIn(str(run_root / "input.wav"), command)
            self.assertIn("--min-stoi", command)

    def test_launch_health_rejects_redbox_but_allows_known_simulator_mic_error(self):
        _assert_app_healthy({"text": ["Mic error — check console"]})
        with self.assertRaisesRegex(Exception, "APP_UNHEALTHY"):
            _assert_app_healthy({"text": ["Unhandled JS Exception", "TypeError"]})

    def test_ignored_turn_is_classified_could_not_run(self):
        self.assertEqual(_failure_status("RELAY_TURN_NOT_ACCEPTED: no transcript"), "could-not-run")
        self.assertEqual(_failure_status("TURN_INCOMPLETE: no assistant response"), "ran-and-failed")

    def test_turn_evidence_requires_all_three_events_and_computes_user_clock_p50(self):
        with tempfile.TemporaryDirectory() as directory:
            trace = Path(directory) / "trace.jsonl"
            records = []
            seq = 0
            for turn, start, latency in (("injected-001", 1.0, 250.0), ("injected-002", 2.0, 350.0)):
                for event, offset in (
                    ("inject.end_of_turn", 0.0),
                    ("stt.final", 100.0),
                    ("assistant.response", 200.0),
                    ("tts.started", 210.0),
                    ("tts.first_audio", latency),
                    ("tts.complete", latency + 50.0),
                ):
                    seq += 1
                    records.append(
                        {
                            "ts": start + offset / 1000.0,
                            "seq": seq,
                            "source": "test",
                            "event": event,
                            "run_id": "run",
                            "session_id": "live-session",
                            "data": {"turn_id": turn},
                        }
                    )
            trace.write_text("".join(json.dumps(record) + "\n" for record in records), encoding="utf-8")
            evidence = _turn_evidence(trace)
            self.assertEqual(evidence["turns_complete"], 2)
            self.assertEqual(evidence["tts_lifecycles_valid"], 2)
            self.assertEqual(evidence["p50_inject_to_first_audio_ms"], 300.0)

    def test_fault_journey_is_not_passed_without_spoken_recovery(self):
        with tempfile.TemporaryDirectory() as directory:
            trace = Path(directory) / "trace.jsonl"
            trace.write_text(
                json.dumps({"seq": 2, "event": "failure", "data": {"kind": "provider"}}) + "\n",
                encoding="utf-8",
            )
            with self.assertRaisesRegex(Exception, "TURN_INCOMPLETE"):
                _assert_spoken_recovery(trace, after_seq=1)

            with trace.open("a", encoding="utf-8") as handle:
                handle.write(json.dumps({"seq": 3, "event": "assistant.response", "data": {}}) + "\n")
                handle.write(json.dumps({"seq": 4, "event": "tts.first_audio", "data": {}}) + "\n")
            _assert_spoken_recovery(trace, after_seq=1)


if __name__ == "__main__":
    unittest.main()
