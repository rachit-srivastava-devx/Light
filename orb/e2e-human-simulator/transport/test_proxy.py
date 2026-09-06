import asyncio
import json
import os
import signal
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from .ws_proxy import ProxyState, TraceWriter, _install_shutdown_handlers, _json


class ProxyUtilityTests(unittest.TestCase):
    def test_json_parser_rejects_non_objects(self):
        self.assertEqual(_json('{"type":"status"}'), {"type": "status"})
        self.assertIsNone(_json("not json"))
        self.assertIsNone(_json(b"{}"))

    def test_trace_writer_emits_evaluator_contract(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "trace.jsonl"
            writer = TraceWriter(path, "run-1")
            writer.emit("harness", "test", {"ok": True})
            writer.close()
            record = json.loads(path.read_text(encoding="utf-8"))
            self.assertEqual(record["seq"], 1)
            self.assertEqual(record["source"], "harness")
            self.assertEqual(record["event"], "test")

    def test_injected_frames_always_use_latest_app_identity(self):
        with tempfile.TemporaryDirectory() as directory:
            writer = TraceWriter(Path(directory) / "trace.jsonl", "run-1")
            state = ProxyState("ws://relay", writer)
            state.observe_app_identity({"tenant_id": "t0", "session_id": "local-1"})
            first, _ = state.injected_frame(
                {"type": "end_of_turn", "tenant_id": "placeholder", "session_id": "local-session"}
            )
            state.observe_app_identity({"tenant_id": "t0", "session_id": "local-2"})
            second, _ = state.injected_frame(
                {"type": "end_of_turn", "tenant_id": "placeholder", "session_id": "local-session"}
            )
            writer.close()
            self.assertEqual(first["session_id"], "local-1")
            self.assertEqual(second["session_id"], "local-2")
            self.assertEqual(first["tenant_id"], "t0")

    def test_injection_without_observed_app_identity_fails_loudly(self):
        with tempfile.TemporaryDirectory() as directory:
            writer = TraceWriter(Path(directory) / "trace.jsonl", "run-1")
            state = ProxyState("ws://relay", writer)
            with self.assertRaisesRegex(RuntimeError, "APP_IDENTITY_UNAVAILABLE"):
                state.injected_frame({"type": "end_of_turn", "session_id": "local-session"})
            writer.close()

    @patch("transport.ws_proxy.subprocess.run")
    def test_byte_identical_consecutive_checkpoints_fail_red(self, run):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)

            def capture(args, **kwargs):
                Path(args[-1]).write_bytes(b"same-png-bytes")
                return type("Completed", (), {"returncode": 0, "stdout": "", "stderr": ""})()

            run.side_effect = capture
            writer = TraceWriter(root / "trace.jsonl", "run-1")
            state = ProxyState(
                "ws://relay",
                writer,
                simulator_udid="sim-1",
                screenshots_dir=root / "screenshots",
            )
            state.snapshot("turn-listening")
            with self.assertRaisesRegex(RuntimeError, "DUPLICATE_CHECKPOINT"):
                state.snapshot("turn-processing")
            writer.close()


class ProxyAcceptanceTests(unittest.IsolatedAsyncioTestCase):
    async def test_turn_acceptance_requires_transcript_response_and_first_audio(self):
        with tempfile.TemporaryDirectory() as directory:
            writer = TraceWriter(Path(directory) / "trace.jsonl", "run-1")
            state = ProxyState("ws://relay", writer, turn_timeout_seconds=0.05)
            state.observe_app_identity({"tenant_id": "t0", "session_id": "live-session"})
            frame, identity = state.injected_frame({"type": "end_of_turn"})
            turn = state.begin_turn(frame, identity)
            state.observe_turn_event(
                "stt.final",
                {"tenant_id": "t0", "session_id": "live-session", "text": "hello"},
            )
            state.observe_turn_event(
                "assistant.response",
                {"tenant_id": "t0", "session_id": "live-session", "text": "hi"},
            )
            state.observe_turn_event(
                "tts.started",
                {"tenant_id": "t0", "session_id": "live-session"},
            )
            state.observe_turn_event(
                "tts.first_audio",
                {"tenant_id": "t0", "session_id": "live-session", "bytes": 3200},
            )
            state.observe_turn_event(
                "tts.complete",
                {"tenant_id": "t0", "session_id": "live-session"},
            )
            accepted = await state.await_turn(turn["turn_id"])
            writer.close()
            self.assertEqual(accepted["type"], "turn_accepted")
            self.assertEqual(accepted["session_id"], "live-session")

    async def test_turn_without_relay_transcript_fails_instead_of_acking(self):
        with tempfile.TemporaryDirectory() as directory:
            writer = TraceWriter(Path(directory) / "trace.jsonl", "run-1")
            state = ProxyState("ws://relay", writer, turn_timeout_seconds=0.01)
            state.observe_app_identity({"tenant_id": "t0", "session_id": "live-session"})
            frame, identity = state.injected_frame({"type": "end_of_turn"})
            turn = state.begin_turn(frame, identity)
            with self.assertRaisesRegex(RuntimeError, "RELAY_TURN_NOT_ACCEPTED"):
                await state.await_turn(turn["turn_id"])
            writer.close()

    async def test_turn_events_must_arrive_in_causal_order(self):
        with tempfile.TemporaryDirectory() as directory:
            writer = TraceWriter(Path(directory) / "trace.jsonl", "run-1")
            state = ProxyState("ws://relay", writer, turn_timeout_seconds=0.01)
            state.observe_app_identity({"tenant_id": "t0", "session_id": "live-session"})
            frame, identity = state.injected_frame({"type": "end_of_turn"})
            turn = state.begin_turn(frame, identity)
            state.observe_turn_event(
                "tts.first_audio",
                {"tenant_id": "t0", "session_id": "live-session", "bytes": 3200},
            )
            self.assertIsNone(state.active_turn["tts_first_audio"])
            with self.assertRaisesRegex(RuntimeError, "RELAY_TURN_NOT_ACCEPTED"):
                await state.await_turn(turn["turn_id"])
            writer.close()

    async def test_identity_change_aborts_an_active_turn(self):
        with tempfile.TemporaryDirectory() as directory:
            writer = TraceWriter(Path(directory) / "trace.jsonl", "run-1")
            state = ProxyState("ws://relay", writer, turn_timeout_seconds=0.01)
            state.observe_app_identity({"tenant_id": "t0", "session_id": "live-session-1"})
            frame, identity = state.injected_frame({"type": "end_of_turn"})
            turn = state.begin_turn(frame, identity)
            state.observe_app_identity({"tenant_id": "t0", "session_id": "live-session-2"})
            with self.assertRaisesRegex(RuntimeError, "APP_IDENTITY_CHANGED"):
                await state.await_turn_stage(turn["turn_id"], "transcript")
            writer.close()


class ProxyShutdownTests(unittest.IsolatedAsyncioTestCase):
    async def test_sigterm_resolves_the_stop_event(self) -> None:
        # run_e2e.py's _terminate() sends SIGTERM to this process. Without a
        # handler, SIGTERM kills the process immediately and skips serve()'s
        # finally (trace.emit/trace.close, any in-flight subprocess cleanup) —
        # the same bug class fixed in simulator/runner.py (SIGTERM vs SIGINT).
        loop = asyncio.get_running_loop()
        stop = asyncio.Event()
        _install_shutdown_handlers(loop, stop)
        try:
            os.kill(os.getpid(), signal.SIGTERM)
            await asyncio.wait_for(stop.wait(), timeout=2)
        finally:
            loop.remove_signal_handler(signal.SIGTERM)
            loop.remove_signal_handler(signal.SIGINT)

    async def test_sigint_resolves_the_stop_event(self) -> None:
        loop = asyncio.get_running_loop()
        stop = asyncio.Event()
        _install_shutdown_handlers(loop, stop)
        try:
            os.kill(os.getpid(), signal.SIGINT)
            await asyncio.wait_for(stop.wait(), timeout=2)
        finally:
            loop.remove_signal_handler(signal.SIGTERM)
            loop.remove_signal_handler(signal.SIGINT)


if __name__ == "__main__":
    unittest.main()
