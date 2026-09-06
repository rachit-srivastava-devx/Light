# Rendered-audio gate

This gate measures the sound that left the Simulator/Mac audio path, not application log lines.
It reports `presence_latency_ms`, `content_latency_ms`, and `presence_only_duration_ms` separately
and fails on any rendered-output gap over `--max-gap-ms` (default **20 ms** — the tripwire from
`blueprints/ADHD-Focus-Orb-L8-Deep-Dive/02-ALWAYS-ON-AUDIO-ENGINE.md` §11.2's loudness-floor probe:
"assert no window > 20 ms falls below the bed's noise floor"; the invariant itself is a 0 ms gap
budget per §1) or when no audio/turn samples were measured.

Capture uses a fixed turn schedule and a stereo artifact: microphone on channel 0, rendered output
on channel 1. The harness rejects missing/off-cadence turns, preventing coordinated omission.

BlackHole is GPL-3 and is local test tooling only. It must never be linked, copied, or bundled into
the product. On iOS 17+ Simulator, do not use Simulator's Audio Output redirect menu; set the Mac
system default output to a Multi-Output Device containing BlackHole plus the listening device.

```bash
scripts/capture-audio-gate.sh 30 0 1 /private/tmp/orb-capture.wav audio-gate/turns.json
```

`gate.py` uses ffmpeg `silencedetect` for continuity and Silero VAD (MIT, local test tooling) for
rendered-speech onset. The gate fails closed when Silero is unavailable. `--content-detector
highpass` exists only for synthetic control fixtures; it is not valid evidence for real speech
because a bright bed morph can cross the same frequency band.

## Failure-mode coverage

| Failure | Current handling and evidence boundary |
| --- | --- |
| Call/Siri or audio-stack reset | Deterministic recovery plan is unit-tested; rendered recovery remains a device test. |
| Background or explicit pause | Bed silence is mandatory and unit-tested through the pause/lifecycle contracts; the continuity gate must not run across an asked-for pause. |
| Bluetooth/headphone route change | Classified as an explicit-pause cause; physical route-change capture is not yet run. |
| Sample-rate mismatch | Capture normalizes both channels to 16 kHz; product playback conversion still needs device evidence. |
| Buffer underrun or GC/main-thread stall | The rendered-output gap gate catches either symptom; no source/log proxy can pass it. |
| Network stall mid-stream | Presence remains on-device; content recovery is owned by the provider pipeline and is not proven here. |
| Provider 402/429/500 | Deliberately not handled here: Track D owns adapter recovery. Presence continuity is still gated independently. |
| TTS returns zero bytes | The bounded provider matrix fails the affected row. |
| Eager transcript superseded | Presence morph reverses on resumed speech; cancellable model work and discard-rate telemetry are not yet exposed by the runtime port. |
| Barge-in during presence-only | **Relay-side yield is now measured** against a live provider (`npm run e2e:bargein`, 2026-08-28: n=10, p50 1.2 ms / p95 1.9 ms vs the 100 ms budget, with 100% of the utterance's bytes suppressed relative to an uninterrupted control; corroborated independently by `scripts/voice-ux-gate.mjs` G5, n=30, p95 1.9 ms). That is transport-level — the relay stops sending. **Actual rendered TTS yield at the speaker within 100 ms is still not measured**, and remains this gate's job: a client with buffered audio can keep playing after the relay has gone quiet, and nothing here or in the e2e harness observes that buffer. |
