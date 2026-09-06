from __future__ import annotations

import json
import signal
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from runner import RunnerError, SimulatorRunner, _handle_termination_signal, command_tokens


class FakeProcess:
    def __init__(self) -> None:
        self.terminated = False
        self.waited = False
        self.signals_received: list[int] = []

    def send_signal(self, sig: int) -> None:
        self.signals_received.append(sig)
        self.terminated = True

    def terminate(self) -> None:
        self.terminated = True

    def wait(self, timeout: int) -> None:
        self.waited = True

    def kill(self) -> None:
        self.terminated = True


def config_for(root: Path, *, create_new: bool = True, replay: bool = True) -> dict:
    return {
        "workspace_root": str(root),
        "artifacts_root": "artifacts",
        "app": {
            "bundle_id": "example.focus.orb",
            "app_path": str(root / "OrbMobile.app"),
        },
        "simulator": {
            "create_new": create_new,
            "device_name": "Orb Test Device",
            "device_type": "com.apple.CoreSimulator.SimDeviceType.iPhone-16-Pro",
            "runtime": "latest",
            "erase_before_boot": True,
            "leave_open": True,
        },
        "commands": {"build": ["echo", "build"], "install": None, "open": None},
        "capture": {
            "log_stream": True,
            "log_predicate": "process == 'OrbMobile'",
            "screenshot": True,
            "video": True,
            "wait_seconds": 0,
        },
        "relay_replay": {
            "enabled": replay,
            "fixture": "fixtures/hello.wav",
            "command": ["replay", "--fixture", "{fixture}", "--handoff", "{replay_handoff}"],
        },
        "evaluators": [],
    }


class SimulatorRunnerTests(unittest.TestCase):
    def test_command_tokens_are_shell_free_and_expanded(self) -> None:
        self.assertEqual(
            command_tokens(["replay", "--fixture", "{fixture}"], {"fixture": "/tmp/a b.wav"}),
            ["replay", "--fixture", "/tmp/a b.wav"],
        )
        self.assertEqual(command_tokens("echo hello", {}), ["echo", "hello"])

    @patch("runner.subprocess.Popen")
    @patch("runner.subprocess.run")
    def test_fresh_visible_run_writes_launch_marker_and_replay_handoff(self, run, popen) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            config = config_for(root)
            processes: list[FakeProcess] = []

            def fake_popen(*args, **kwargs):
                process = FakeProcess()
                processes.append(process)
                return process

            def fake_run(args, **kwargs):
                if args == ["xcrun", "simctl", "list", "runtimes", "available", "--json"]:
                    return _completed(args, json.dumps({"runtimes": [{"identifier": "com.apple.CoreSimulator.SimRuntime.iOS-18-0", "version": "18.0", "isAvailable": True}]}))
                if args[:3] == ["xcrun", "simctl", "create"]:
                    return _completed(args, "fresh-udid\n")
                if args[:3] == ["xcrun", "simctl", "launch"]:
                    return _completed(args, "example.focus.orb: pid 4242\n")
                if args == ["xcrun", "simctl", "help", "io"]:
                    return _completed(args, "screenshot\nrecordVideo\n")
                return _completed(args)

            run.side_effect = fake_run
            popen.side_effect = fake_popen
            artifacts = SimulatorRunner(config, root / "manifest.json").run_session(wait_seconds=0)

            marker = json.loads((artifacts / "launch-result.json").read_text())
            handoff = json.loads((artifacts / "replay-handoff.json").read_text())
            self.assertEqual(marker["simulator_udid"], "fresh-udid")
            self.assertEqual(marker["app_pid"], 4242)
            self.assertEqual(marker["screenshot_path"], str(artifacts / "launch.png"))
            self.assertEqual(marker["log_path"], str(artifacts / "simulator.log"))
            self.assertEqual(handoff["fixture_path"], str((root / "fixtures/hello.wav").resolve()))
            self.assertEqual(handoff["simulator_udid"], "fresh-udid")
            self.assertEqual(len(processes), 2)
            self.assertTrue(all(process.terminated and process.waited for process in processes))
            # Regression guard: SIGTERM only kills the CLI wrapper and leaves the
            # CoreSimulator daemon recording forever (confirmed against a real
            # simulator). Capture must be stopped with SIGINT.
            self.assertTrue(all(process.signals_received == [signal.SIGINT] for process in processes))

            commands = [call.args[0] for call in run.call_args_list]
            self.assertIn(["xcrun", "simctl", "install", "fresh-udid", str(root / "OrbMobile.app")], commands)
            self.assertIn(["xcrun", "simctl", "launch", "fresh-udid", "example.focus.orb"], commands)
            self.assertIn(["replay", "--fixture", str((root / "fixtures/hello.wav").resolve()), "--handoff", str(artifacts / "replay-handoff.json")], commands)

    @patch("runner.subprocess.Popen")
    @patch("runner.subprocess.run")
    def test_existing_device_is_not_shutdown_or_erased(self, run, popen) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            config = config_for(root, create_new=False, replay=False)

            def fake_run(args, **kwargs):
                if args == ["xcrun", "simctl", "list", "devices", "available", "--json"]:
                    return _completed(args, json.dumps({"devices": {"iOS 18": [{"name": "Orb Test Device", "udid": "existing-udid"}]}}))
                if args[:3] == ["xcrun", "simctl", "launch"]:
                    return _completed(args, "example.focus.orb: pid 11\n")
                if args == ["xcrun", "simctl", "help", "io"]:
                    return _completed(args, "screenshot\nrecordVideo\n")
                return _completed(args)

            run.side_effect = fake_run
            SimulatorRunner(config, root / "manifest.json").run_session(wait_seconds=0)
            commands = [call.args[0] for call in run.call_args_list]
            self.assertNotIn(["xcrun", "simctl", "shutdown", "existing-udid"], commands)
            self.assertNotIn(["xcrun", "simctl", "erase", "existing-udid"], commands)
            self.assertIn(["xcrun", "simctl", "boot", "existing-udid"], commands)
            popen.assert_called()

    @patch("runner.subprocess.run")
    def test_failed_command_is_reported_and_does_not_continue(self, run) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            config = config_for(root, replay=False)
            run.return_value = _completed(["echo", "build"], returncode=7, stderr="build failed")
            with self.assertRaises(RunnerError):
                SimulatorRunner(config, root / "manifest.json").run_session(wait_seconds=0)
            self.assertEqual(run.call_count, 1)

    def test_termination_signal_raises_runner_error(self) -> None:
        # A hung build/install/replay command must not be able to swallow a
        # SIGTERM/SIGHUP silently: it has to surface as a RunnerError so
        # run_session's `finally` still stops the video/log capture.
        with self.assertRaises(RunnerError):
            _handle_termination_signal(signal.SIGTERM, None)

    @patch("runner.subprocess.Popen")
    @patch("runner.subprocess.run")
    def test_run_session_installs_and_restores_termination_handlers(self, run, popen) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            config = config_for(root, replay=False)
            original_sigterm = signal.getsignal(signal.SIGTERM)
            original_sighup = signal.getsignal(signal.SIGHUP)

            def fake_run(args, **kwargs):
                if args[:3] == ["xcrun", "simctl", "create"]:
                    return _completed(args, "fresh-udid\n")
                if args[:3] == ["xcrun", "simctl", "launch"]:
                    return _completed(args, "example.focus.orb: pid 4242\n")
                if args == ["xcrun", "simctl", "help", "io"]:
                    return _completed(args, "screenshot\nrecordVideo\n")
                if args == ["xcrun", "simctl", "list", "runtimes", "available", "--json"]:
                    return _completed(args, json.dumps({"runtimes": [{"identifier": "com.apple.CoreSimulator.SimRuntime.iOS-18-0", "version": "18.0", "isAvailable": True}]}))
                # While a command is "in flight", the installed handler must not
                # be Python's default (process-killing) disposition.
                self.assertEqual(signal.getsignal(signal.SIGTERM), _handle_termination_signal)
                self.assertEqual(signal.getsignal(signal.SIGHUP), _handle_termination_signal)
                return _completed(args)

            run.side_effect = fake_run
            SimulatorRunner(config, root / "manifest.json").run_session(wait_seconds=0)

            self.assertEqual(signal.getsignal(signal.SIGTERM), original_sigterm)
            self.assertEqual(signal.getsignal(signal.SIGHUP), original_sighup)

    @patch("runner.subprocess.Popen")
    @patch("runner.subprocess.run")
    def test_command_timeout_stops_owned_processes(self, run, popen) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            config = config_for(root, replay=False)
            config["command_timeout_seconds"] = 5
            processes: list[FakeProcess] = []

            def fake_popen(*args, **kwargs):
                process = FakeProcess()
                processes.append(process)
                return process

            def fake_run(args, **kwargs):
                if args == ["xcrun", "simctl", "list", "runtimes", "available", "--json"]:
                    return _completed(args, json.dumps({"runtimes": [{"identifier": "com.apple.CoreSimulator.SimRuntime.iOS-18-0", "version": "18.0", "isAvailable": True}]}))
                if args[:3] == ["xcrun", "simctl", "create"]:
                    return _completed(args, "fresh-udid\n")
                if args == ["xcrun", "simctl", "help", "io"]:
                    return _completed(args, "screenshot\nrecordVideo\n")
                if args[:3] == ["xcrun", "simctl", "launch"]:
                    raise subprocess.TimeoutExpired(cmd=args, timeout=5)
                return _completed(args)

            run.side_effect = fake_run
            popen.side_effect = fake_popen
            with self.assertRaises(RunnerError):
                SimulatorRunner(config, root / "manifest.json").run_session(wait_seconds=0)

            # The log + video capture subprocesses were already started before
            # the hung "launch" command; they must still be torn down.
            self.assertEqual(len(processes), 2)
            self.assertTrue(all(process.terminated and process.waited for process in processes))


def _completed(args, stdout: str = "", stderr: str = "", returncode: int = 0):
    from subprocess import CompletedProcess

    return CompletedProcess(args, returncode, stdout=stdout, stderr=stderr)


if __name__ == "__main__":
    unittest.main()
