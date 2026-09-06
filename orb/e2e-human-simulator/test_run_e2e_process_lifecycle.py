import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from run_e2e import E2EError, _run_replay, _start


class RunReplayTimeoutTests(unittest.TestCase):
    @patch("run_e2e.subprocess.run")
    def test_hung_replay_raises_instead_of_blocking_forever(self, run) -> None:
        # A stuck replay (dead socket recv, backend deadlock) must not block the
        # whole orchestrator forever before it reaches the caller's cleanup of
        # owned processes (backend/proxy) — the same shape as the SIGTERM/SIGINT
        # bug fixed in simulator/runner.py.
        #
        # CPython can hand back TimeoutExpired.stdout/stderr as raw bytes even
        # with text=True; assert with bytes here so a naive `exc.stdout or ""`
        # regression (which crashed on real timeouts before) is caught.
        run.side_effect = subprocess.TimeoutExpired(cmd=["replay"], timeout=120, output=b"partial", stderr=b"partial-err")
        with tempfile.TemporaryDirectory() as directory:
            run_root = Path(directory)
            with self.assertRaises(E2EError):
                _run_replay(
                    base=run_root,
                    env={},
                    run_root=run_root,
                    control_url="ws://127.0.0.1:1",
                    fixture=Path("fixture.wav"),
                    journey_id="j1",
                    step_index=0,
                    session_id="s1",
                    speed=1.0,
                    post_turn_wait=0.0,
                    observation="",
                )
            self.assertTrue((run_root / "replay-j1-00.log").exists())


class StartProcessTests(unittest.TestCase):
    @patch("run_e2e.subprocess.Popen")
    def test_log_handle_is_closed_even_when_popen_raises(self, popen) -> None:
        # If Popen raises after the log file is opened (e.g. command not found),
        # the handle must still be closed instead of leaking a file descriptor
        # for the rest of the orchestrator's lifetime.
        popen.side_effect = FileNotFoundError("no such command")
        opened_handles: list = []
        real_open = Path.open

        def tracking_open(self, *args, **kwargs):
            handle = real_open(self, *args, **kwargs)
            opened_handles.append(handle)
            return handle

        with tempfile.TemporaryDirectory() as directory:
            log_path = Path(directory) / "backend.log"
            with patch.object(Path, "open", tracking_open):
                with self.assertRaises(FileNotFoundError):
                    _start(["does-not-exist"], cwd=Path(directory), env={}, log_path=log_path)

        self.assertEqual(len(opened_handles), 1)
        self.assertTrue(opened_handles[0].closed)


if __name__ == "__main__":
    unittest.main()
