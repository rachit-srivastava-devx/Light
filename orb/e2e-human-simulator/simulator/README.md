# Visible iOS Simulator runner

`runner.py` is an independent black-box harness. It does not import the app,
React Native, relay, gateway, or provider code. Its only product boundary is
the configured command line and `xcrun simctl`.

## Run a visible session

From this directory:

```bash
python3 runner.py --config manifest.json --wait-seconds 30
```

The runner will:

1. Build with the configured command.
2. Create a new available iOS Simulator by default and boot it.
3. Install the configured `.app` and launch the configurable bundle ID.
4. Keep the simulator GUI open after the runner exits.
5. Capture a simulator log stream, screenshot, and video when supported.
6. Optionally create a deterministic relay-replay handoff and run a configured replay command.
7. Optionally run configured Codex/Claude evaluator commands over the artifacts.

The discovered project bundle ID is currently `org.reactjs.native.example.OrbMobile`,
but it remains configurable in `manifest.json`. The runner never assumes that ID
in code.

Set `simulator.show_window` to `true` to bring the Xcode Simulator GUI to the foreground during
the run. Screenshot files remain the authoritative artifact if macOS Assistive Access prevents
host UI inspection.

`manifest.json` can explicitly grant simulator permissions, for example
`"permissions": ["microphone"]`. The runner still captures the launch screenshot: a visible
permission dialog is evidence of a blocked first-run journey, not a passing permission setup.

After launch, the terminal prints a machine-readable marker:

```text
=== SIMULATOR_LAUNCH_RESULT ===
{"app_pid":1234,"app_bundle_id":"...","log_path":".../simulator.log","screenshot_path":".../launch.png","simulator_udid":"...","video_path":".../session.mp4"}
```

The same marker is saved as `launch-result.json` in the run artifact directory.

## Deterministic microphone replay handoff

CoreSimulator does not provide a general `simctl` command that injects an
arbitrary host WAV file into an app's `AVAudioEngine` microphone input. This
runner therefore does not pretend that a screenshot plus `simctl` launch is a
microphone test.

Set `relay_replay.enabled` to `true` and configure `relay_replay.command` for a
separate replay process. Before invoking it, the runner writes
`replay-handoff.json` containing:

- absolute fixture path;
- simulator UDID and bundle ID;
- run artifact directory;
- required frame timing, transport, and evidence contract.

The replay process owns WAV decoding and must convert the fixture into the same
PCM/frame protocol used by the relay. It should write transcript, backend
response, frame timing, and errors under `{artifacts_dir}`. The runner only
orchestrates the handoff; it does not import or call app code.

## Safety and evidence

- A fresh simulator is created by default, so erase/shutdown applies only to a
  simulator created by this run.
- If `create_new` is set to `false`, an existing device is never implicitly
  shut down or erased.
- The runner terminates only its own log/video capture subprocesses. It does
  not enumerate, kill, or shut down unrelated processes.
- Cleanup of those owned subprocesses is guaranteed, not just attempted on a
  clean exit: every `xcrun`/build/install/replay/evaluator command is bounded
  by `command_timeout_seconds` (default 900s, override or set to `0`/`null` to
  disable), and SIGTERM/SIGHUP are converted into a `RunnerError` so a
  cancelled or terminated run still unwinds through the same cleanup path as
  a normal failure. Without this, a hung command or a killed runner process
  leaves `recordVideo` writing to `session.mp4` with nothing left to stop it —
  the failure mode that produced a 69GB orphaned recording in this repo.
- Stopping `recordVideo` (and the log stream) sends **SIGINT**, not SIGTERM:
  `simctl io ... recordVideo` only finalizes and releases the file on SIGINT
  (the same signal an interactive Ctrl-C sends). SIGTERM just kills the CLI
  wrapper — verified against a real simulator that the CoreSimulator daemon
  keeps recording to the file indefinitely afterward and reports "Host
  recording is already in progress" for any new attempt on that device.
  SIGTERM/SIGKILL are still used as an escalation if SIGINT doesn't finish
  within 10s.
- Every synchronous command is recorded in `commands.jsonl`; `report.md` is a
  human-review checklist, not an assertion that the experience felt correct.
- Codex/Claude are optional post-run evaluators. They cannot change the hard
  deterministic outcome and are disabled by default.

## Tests

The tests mock `subprocess.run` and `subprocess.Popen`; they never require an
iOS Simulator and never execute `xcrun`:

```bash
python3 -m unittest -v test_runner.py
```
