# ADR 0005 — Final verifier loop and manual smoke evidence

**Status:** accepted. **Date:** 2026-08-04.

## Context

The final completion pass added the remaining Phase 1 surfaces that were still placeholders or
absent: client VAD/check-in/evidence/voice-routing logic, backend warmup/cache/eval/observability
surfaces, the product manifest, junior traversal docs, native React Native hosts, the Rust realtime
provider contract, and the WebSocket voice smoke.

The requested worker-agent fanout was attempted, but the worker host returned a usage-limit error
for each spawned worker before it could edit files. The orchestrator completed the same disjoint
scopes locally and kept the same verification requirements.

## Programmatic verification

Command:

```sh
PATH=/Users/rachitsrivastava/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin /opt/homebrew/bin/npm run verify
```

Result:

- `boundary-lint`: clean
- TypeScript typecheck: pass
- Vitest: 25 files, 282 tests passed
- Python pytest: 51 tests passed, 1 Starlette/httpx deprecation warning
- Rust cargo tests: 24 tests passed

## Manual smoke

Started the T0 sidecar + live relay with the executable verifier:

```sh
npm run smoke:t0
```

The verifier drove the HTTP surface:

- sidecar `GET /healthz` -> `200 {"status":"ok"}`
- relay `GET /healthz` -> `200 {"status":"ok"}`
- `POST /v1/session/warmup` -> `200` with `profile`, empty T0 context arrays, `open_session`, and
  `phrase_manifest`
- `POST /v1/cache/prime` -> `200 {"primed":true,"source":"t0:atomizer.v4"}`
- `POST /v1/eval/metrics` with no caller metrics -> `200 {"passed":true,...}` for the local T0
  replay suite: deterministic expansions to 300 atomizer cases, 200 voice cases, 200 consistency
  cases, 500 classifier utterances, 5,000 belief windows, and 200 cost/latency/policy-behavior
  replays. The evidence boundary still excludes live provider and physical-device audio quality.
- `POST /v1/atomize` through the T0 memory sidecar -> `200` deterministic fallback

The no-sidecar failure path remains covered by `backend/relay-py/tests/test_app_routes.py`.

Additional manual/runtime checks:

- `npm run smoke:realtime` passed against the Rust WebSocket relay with tenant/session IDs,
  transcript routing, `speech_starting`, two binary TTS chunks, and `speech_complete`.
- `npm run mobile:bundle` passed through Metro from the native root `index.js`.
- `npm run android:build:check` now packages `index.android.bundle` locally, then produces a debug
  APK through Gradle/NDK/CMake.
- `ORB_STRICT_AUDIO_BUDGETS=1 npm run smoke:android:e2e` passed on `FocusOrb_API36`: built the APK, installed it, launched
  `com.orbmobile/.MainActivity`, waited for the real UI, asserted exactly the orb-only app surface
  with no visible app text and no clickable app controls, and verified native speech queueing through
  `OrbGreeting: queued`, `OrbSpeech: queued`, and `focus-orb:speak Open the first small action`
  logcat evidence. Captured budgets: `presence_ready=0ms/120ms`,
  `speech_request=0ms/250ms`. Screenshot: `/tmp/focus-orb-android-orb-only.png`; durable
  evidence: `docs/evidence/android-e2e/`.
- `xcodebuild -downloadPlatform iOS -buildVersion 26.5 -architectureVariant arm64` installed the
  iOS 26.5 simulator runtime.
- `npm run native:preflight` still fails only on CoreSimulator health: `xcrun simctl` times out.
- `npm run ios:build:check` still fails on this host because Xcode 26.6 reports CoreSimulator
  1051.54.0, while Xcode requires 1051.55.0. `softwareupdate --list` reports macOS Tahoe 26.6
  available with a required restart.
- A short Claude Sonnet review returned NO-GO until the iOS host update/restart and a real
  device/audio pass are completed.

## Reviewer verdict

Go-ahead for the Android/T0 executable slice based on:

- C1/C9/C12 preserved: model calls remain behind the gateway sidecar, cost surfaces stay in-path,
  and no provider SDK import was added outside the allowed sidecar.
- L8 readability preserved: new logic is small, pure where possible, threshold constants are named,
  and failure states are typed or explicit.
- Prior lessons checked: missing/malformed/threshold tests were added for new branch-shaped logic;
  no optional-default branch silently chooses a success state; route/manual smoke covers the
  injected-transport seam from LESSONS L8.
- Manual verification was performed against the running relay and Android emulator rather than
  trusting unit tests only.

No-go for claiming complete Phase 1 release until the iOS host/CoreSimulator mismatch is fixed and
the real iOS or physical-device audio path is manually verified. This ADR must not be cited as a
full release approval while that blocker remains.
