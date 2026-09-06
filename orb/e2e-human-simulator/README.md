# Focus Orb independent human-simulation harness

This folder is an external black-box test tool. It does not import the Focus Orb app, its
TypeScript modules, or its backend implementation. It talks to the product only through the
same observable surfaces a user and an operator have:

- iOS Simulator lifecycle through `xcrun simctl`;
- relay WebSocket/HTTP endpoints configured in `config.json`;
- WAV/PCM microphone frames;
- simulator screenshots and process logs;
- JSONL evidence traces.

The harness has three independent parts:

```text
replay/    deterministic microphone fixtures and frame timing
simulator/ visible simulator boot, install, launch, screenshots, logs
evaluator/ deterministic checks, judge adapters, and felt-wrong report
```

The evaluator is never in the control loop. It receives captured evidence after a journey has
finished. Codex/Claude may score that evidence, but a judge cannot make the app pass a hard
contract failure such as a missing transcript, invalid state transition, duplicate reply, or
provider error.

## Run shape

```text
prepare fresh simulator
        ↓
launch configured app visibly
        ↓
start real backend services separately
        ↓
replay WAV as microphone-sized PCM frames
        ↓
capture state, transcript, gateway, TTS, logs, screenshots
        ↓
run deterministic checks
        ↓
optional faster-whisper/JiWER/pYSTOI/NISQA audio gate
        ↓
run Codex/Claude over evidence only
        ↓
write report.md and machine-readable verdicts
```

The target app, build command, bundle identifier, relay URLs, simulator ID, and provider mode
are configuration. No target source path is imported by this harness.

## Evidence standard

Each run must leave an immutable run directory containing:

```text
input/*.wav
trace.jsonl
screenshots/
logs/
deterministic-verdict.json
judge-verdict.json
report.md
```

A run is not called end-to-end merely because a unit test passed. The report must identify the
journey, exact input fixture, observed transcript, response, latency, orb state timeline, and any
failure that a human would feel.

## One-command visible run

Copy `run_config.example.json` to a local config, point each journey at a real WAV fixture, and
run:

```bash
PYTHONPATH=. python3 run_e2e.py --config run_config.json
```

Use `--no-judges` for fast iteration. Omit it for the final run so local Codex and Claude review
the captured evidence after the app journey. A journey may use `fault` instead of `wav` to inject
a protocol-level provider failure into the app's live socket and evaluate spoken recovery.

The orchestrator starts only explicitly configured backend processes, opens the configured fresh
simulator, leaves it open, replays fixtures through the app's own WebSocket, captures launch/
listening/processing/complete screenshots, and runs deterministic evaluation followed by optional
local Codex/Claude review. Every run is kept under one `runs/<run-id>/` directory. An evaluator
FAIL is a useful result: it means the visible human journey exposed a requirement violation.

To include the model-backed audio gate in an orchestrated run, set
`audio_quality.enabled` to `true` in the run config and provide the captured
microphone WAV, captured playback WAV, same-content clean TTS reference, local
faster-whisper model, and local NISQA-TTS checkpoint. The orchestrator writes
`audio-quality.json` and `audio-quality.log` into the run directory and returns
failure if any audio layer is blocked or below threshold.

## Barge-in measurement (`barge_in_drive.mjs`)

Barge-in had never been measured in this product before this script existed. Three independent
signals said so: `scripts/voice-ux-gate.mjs`'s G5 found ~1 barge-in dispatch sample across the
whole `dev-logs/` corpus (tens of thousands of rows) and fails loudly rather than report a
meaningless 100% off one row; `audio-gate/README.md` states outright that rendered barge-in yield
"is not yet measured"; every barge-in screenshot in this directory's own `runs/**/trace.jsonl` is
tagged `"ui_confirmed": false`.

```bash
npm run e2e:bargein                                  # 4 frame types x 10 samples + control (default)
BARGE_IN_FRAME_TYPES=barge_in npm run e2e:bargein     # cheap, one path only
BARGE_IN_SAMPLES=20 npm run e2e:bargein               # more samples per frame type
node e2e-human-simulator/barge_in_drive.test.mjs      # standalone unit tests (no live services, no cost)
```

**What it measures, precisely:** connects to real relay-rs over its real WebSocket
(`ws://127.0.0.1:8091`), makes it speak with real Fish TTS audio, sends an interruption control
frame, and times — on this script's own clock — the interval from that send to the arrival of the
LAST binary audio frame the in-flight `speak` produces. That number is **relay-side frame-stop
latency, a transport-level measurement. It is NOT audio-yield-at-the-speaker.** A real barge-in has
three layers — (a) the relay stops sending frames, (b) the client stops playback, (c) the speaker
goes quiet — and this script covers only (a). (b) is owned by `apps/mobile/src/voice/BargeIn.ts` /
`RelayAudioPlayer.ts` (out of scope, DO-NOT-EDIT-adjacent) and is not exercised at all. (c) is
`audio-gate/`'s acknowledged, still-open gap (mic/speaker capture, not logs) and is not attempted
here. The script prints this same breakdown, in full, at the end of every run.

**Frame types tested**, grounded in `backend/relay-rs/src/protocol.rs` (not guessed): `barge_in`
and `pause` are the only two the protocol documents as interruption-shaped; `end_of_turn` and
`start_listening` are STT turn-lifecycle frames, tested anyway because sending them mid-speech is a
fact worth having on the record rather than assumed.

**Why two interrupt-timing conditions:** a first pilot run that fired the interrupt on the first
*observed* audio frame measured latencies of essentially 0ms — not because the relay is fast, but
because `main.rs`'s unpaced `for chunk in audio_chunks { sink.send(...).await? }` loop delivers an
entire multi-hundred-KB reply in under one JS event-loop tick on loopback, so there is no
multi-hundred-millisecond "streaming" window to land an interrupt inside of on this transport. The
real bottleneck is `tts.synthesize()` itself (2-3.4 real seconds of synchronous, unyielding HTTP
wait per the measured run). So the **primary** condition (`immediate`) fires the interrupt ~50ms
after `speak`, while the relay is provably still blocked in synthesis — the only regime where
"yield" could mean anything. The **secondary** condition (`first_audio`, barge_in only, smaller
sample) is kept specifically to keep demonstrating the sub-ms-full-burst finding on every re-run.
Full reasoning is in the block comment above `runTrial()` in the source — read it before changing
the timing logic.

**Fail-closed:** zero audio frames across every trial, or fewer than 10 usable samples for any
frame type, exits 6 before any PASS/FAIL judgement is even attempted. barge_in's own p95 exceeding
the product's 100ms budget (`backend/relay-rs/src/latency_budget.rs:18`,
`blueprints/ADHD-Focus-Orb-L8-Deep-Dive/03-VOICE-LATENCY-PIPELINE.md:167`, `AGENTS.md:88`,
`CLAUDE.md:43`) also exits 6. A run that measures nothing, or too little to trust, must never
report success.

**Cost:** real Fish TTS credit, computed from characters synthesized at this product's own
configured rate (`backend/relay-py/src/orb_relay/app.py:229`, 143 paise/1000 chars) and printed at
the end of every run. The default run (50 `speak` calls) costs roughly ₹12 (~$0.13).

**Bonus:** after a run, `node scripts/voice-ux-gate.mjs` will show G5 with a real, non-trivial
`barge_in` denominator instead of ~1 — this script's trials write to the same `dev-logs/` corpus
G5 reads, via the real relay's own logging, not anything this script writes itself.
