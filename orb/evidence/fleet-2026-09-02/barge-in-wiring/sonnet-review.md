# A8 barge-in wiring — mid-engineer review and completion (Sonnet)

Date: 2026-09-02
Branch: `fix/barge-in-wiring`
Base: `e942446`

Codex (an earlier agent on this task) ran out of usage quota mid-task and left real,
uncommitted work in this worktree. This file documents what I (Sonnet, mid-engineer)
independently reviewed, what I found already correct, the one gap I fixed, and the full
verification I re-ran from scratch.

## What codex had already done (verified by reading every diff line, not assumed)

1. `apps/mobile/src/voice/BargeIn.ts` — added `BargeInObservation`, `BargeInCoordinator`,
   `BargeInEffects`, and `createBargeInCoordinator(effects)`. This turns a stream of post-AEC
   mic/VAD frames into a one-shot cancellation edge: it tracks `candidateStartedAtMs`, requires
   `VAD_SPEECH_PROBABILITY` (0.6) to open a candidate and only `VAD_SILENCE_PROBABILITY` (0.35) to
   sustain it (hysteresis matching `VADGate.ts`'s own pattern), resets on `orb_is_speaking=false` or
   `is_echo_residue`, and calls `effects.cancelProviderCall(BARGE_IN_YIELD_BUDGET_MS)` exactly once
   per speaking turn the instant `decideBargeIn` (the pre-existing pure function) says `yield: true`
   at 200ms of sustained speech (`BARGE_IN_MIN_SPEECH_MS`).
2. `apps/mobile/src/runtime/VoiceLoopController.ts` — wires a `bargeInCoordinator` instance whose
   `cancelProviderCall` effect is `() => relay?.bargeIn()` (the existing transport method that sends
   the `barge_in` socket frame relay-rs already knows how to act on — see below). Adds
   `handleBargeInObservation(input)` to the controller's public interface and resets the coordinator
   in the existing turn-reset path.
3. `apps/mobile/src/App.tsx` — the native mic callback now calls
   `voiceLoop.handleBargeInObservation(...)` on every frame (including silent-orb frames, so the
   coordinator's one-shot latch resets correctly turn-to-turn), and when the orb is speaking and a
   `barge_in_yield` event comes back, stops the retained local player
   (`relayAudioPlayer.current?.handleClosing('user_pause')`), cancels queued presence events, clears
   `assistantSpeakingRef`, and restores bed voice gain.
4. `apps/mobile/src/voice/BargeInPlaybackLatency.test.ts` — added a
   `describe('detected barge-in cancels provider work', ...)` block with two tests: one driving
   `VoiceLoopController.handleBargeInObservation` end-to-end to the injected relay `bargeIn()` port,
   and one driving `createBargeInCoordinator` directly with an `AbortController`-backed fake provider
   call, asserting the call is aborted within the `BARGE_IN_YIELD_BUDGET_MS` budget and that the
   provider promise actually rejects.
5. `apps/mobile/src/AppVoiceWiring.test.tsx` — added `FakeWebSocket.deliverSpeechStarting()` and one
   integration test (`'sends provider cancellation when the mic detects sustained speech over an
   in-flight reply'`) that mounts the real `App.tsx`, delivers a `speech_starting` server frame (no
   audio chunk), feeds two mic frames 200ms apart at `speech_probability: 0.95`, and asserts the
   socket recorded exactly one `barge_in` control frame.
6. Backend `backend/relay-rs` was **not** touched (confirmed via `git status` — no diff). Reading
   `session.rs`'s `barge_in()` function and `main.rs`'s `barge_in_during_synthesis_delivers_no_audio_at_all`
   test confirms the server-side cancellation (retiring the live generation, polling the provider
   cancel flag every 25ms) already existed before this task. The actual A8 defect was that the
   client never sent `barge_in` on the fast VAD-onset path — only on the slower final-transcript
   `classifyConversationControl(text) === 'stop'` path (documented in the pre-diff top-of-file
   comment codex left as a paper trail). This diff is exactly the missing wiring.

All of this was correct and complete except one gap (below).

## Gap I found and fixed

`App.tsx`'s `onSpeechStarting` handler (the relay's confirmation that TTS synthesis has begun)
did **not** set `assistantSpeakingRef.current = true`. In production this doesn't matter today,
because the local `speak(text, ...)` function already sets that ref synchronously at request time,
before the relay's `speech_starting` frame can arrive. But codex's own new `AppVoiceWiring.test.tsx`
test exercises `onSpeechStarting` **without** first calling the app's local `speak()` — it injects
`speech_starting` directly via the fake socket. Tracing `orbIsSpeaking =
assistantSpeakingRef.current || relayAudioPlayer.current?.isPlaying()` shows that under the
as-left diff this evaluates to `false` in that test (no audio chunk delivered either), so the mic
frames would fall through to normal processing and never reach the cancellation port — the new test
would fail once it could actually execute (see harness limitation below).

Fix (`apps/mobile/src/App.tsx`, `onSpeechStarting`): also set `assistantSpeakingRef.current = true`
there, idempotently. This is defensible independent of the test: it makes "orb is speaking" for
barge-in purposes derive from the relay's own confirmation, not only local intent, which is strictly
more robust (e.g. if a future code path ever requests speech through some entry point other than the
local `speak()` wrapper, or if an error handler ever clears the ref between request and confirmation).

```diff
       onSpeechStarting: () => {
+        // Local `speak()` already sets this synchronously when this app requested the utterance;
+        // this is belt-and-braces so barge-in detection reads "orb is speaking" off the relay's
+        // own confirmation too, not only local intent, in case that flag was ever cleared (e.g. by
+        // an error handler) between the request and this confirmation arriving.
+        assistantSpeakingRef.current = true;
         setRelayStatus('ready');
         setSpeechStatus('speaking');
```

## Harness limitation this fix could not be run against directly

`AppVoiceWiring.test.tsx` mounts the real `App.tsx`, which imports `./runtime/T0FocusSession`,
which imports `@pe/realtime-voice` and `@pe/realtime-voice/adapters/memory`. In this worktree
(and confirmed identically in a disposable detached `git worktree add --detach <scratch> e942446`
of the unmodified base, see `verify.txt` / re-confirmed below) that resolves to a symlink
(`node_modules/@pe/realtime-voice -> ../../../../../registry/features/realtime-voice`) whose
relative depth only matches the *original* checkout path (`company/products/adhd-focus-orb`), not
a worktree nested one level deeper (`company/products/adhd-focus-orb-worktrees/<name>`). This is
the same `@pe/realtime-voice`/`@pe/voice-realtime` gap this session has hit before — pre-existing,
structural to how worktrees are laid out, and unrelated to A8. It blocks running
`AppVoiceWiring.test.tsx`, `VoiceLoopController.test.ts`, `T0FocusSession.test.ts`, and (via the
sibling `@pe/llm-gateway` package) two `backend/gateway-sidecar` test files, in every worktree under
`adhd-focus-orb-worktrees/`, with or without this diff.

Because I could not execute the new `AppVoiceWiring.test.tsx` test, I verified it by code trace
only (above), not by RED→GREEN execution. Everything I *could* execute is RED→GREEN verified below.

## What I independently re-verified (fresh commands, this session)

### 1. Direct unit-level owner of the A8 property: GREEN, 29/29

```
$ npx vitest run apps/mobile/src/voice/BargeInPlaybackLatency.test.ts
 ✓ apps/mobile/src/voice/BargeInPlaybackLatency.test.ts (29 tests) 32ms
 Test Files  1 passed (1)
      Tests  29 passed (29)
```

(Codex's own `red.txt`/`green.txt` in this directory show 1/28 RED before `createBargeInCoordinator`
existed and 29/29 GREEN after — I re-ran the GREEN case myself and reproduced it, unmodified.)

### 2. `npm run verify` — exit 2, halts at typecheck on the pre-existing gap only

```
$ npm run verify
> npm run lint && npm run typecheck && npm run test && npm run test:py && npm run test:rs
> node tooling/boundary-lint.mjs
boundary-lint: clean
> tsc --noEmit -p apps/mobile && tsc --noEmit -p backend/gateway-sidecar && tsc --noEmit -p backend/voice-provider-sidecar
apps/mobile/src/runtime/T0FocusSession.ts(1,66): error TS2307: Cannot find module '@pe/realtime-voice' or its corresponding type declarations.
apps/mobile/src/runtime/T0FocusSession.ts(2,50): error TS2307: Cannot find module '@pe/realtime-voice/adapters/memory' or its corresponding type declarations.
```
Exit code: 2. 15 lines of output total — exactly these 2 errors, nothing else.

### 3. Isolation: identical failure on an unmodified `e942446` in a disposable detached worktree (not stash)

```
$ git worktree add --detach <scratch>/verify-baseline2 e942446
$ ln -s <this-worktree>/node_modules <scratch>/verify-baseline2/node_modules
$ cd <scratch>/verify-baseline2 && npm run verify
```
Byte-for-byte the same 2 `TS2307` errors, same exit code 2, same 15-line output. Confirms the
typecheck failure is pre-existing and unrelated to this diff. (Cleaned up afterward with
`git worktree remove --force`.)

### 4. Full `vitest run` (bypassing the `&&`-chained `npm run verify` halt) — no regressions, 2 new passing tests, same 6 broken suites in both trees

Modified worktree:
```
 Test Files  6 failed | 48 passed (54)
      Tests  618 passed (618)
```
Unmodified `e942446` baseline (same detached worktree, same linked `node_modules`):
```
 Test Files  6 failed | 48 passed (54)
      Tests  616 passed (616)
```
The 6 failed suites are identical in both trees, all `Cannot find module '@pe/realtime-voice...'`
or `'@pe/llm-gateway...'` import errors (never a test assertion failure):
`AppSurface.test.tsx`, `AppVoiceWiring.test.tsx`, `T0FocusSession.test.ts`,
`VoiceLoopController.test.ts`, `backend/gateway-sidecar/__tests__/adapterTelemetry.test.ts`,
`backend/gateway-sidecar/__tests__/complete.test.ts`. The 618 vs 616 delta is exactly the 2 new
tests added to `BargeInPlaybackLatency.test.ts`. Zero regressions; zero new failures.

### 5. Backend relay-rs in-flight cancellation test — still green (unchanged, re-run to confirm)

```
$ cd backend/relay-rs && cargo test --quiet barge_in_during_synthesis_delivers_no_audio_at_all
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 56 filtered out
```

## Files changed (this session, on top of codex's diff)

- `apps/mobile/src/App.tsx` — added `assistantSpeakingRef.current = true;` inside `onSpeechStarting`
  (one line + comment). No other change.

## Reproduce from a fresh clone

```
git clone <repo> && cd <repo>
git checkout fix/barge-in-wiring
npm install
npx vitest run apps/mobile/src/voice/BargeInPlaybackLatency.test.ts   # expect 29/29 green
npm run verify                                                        # expect exit 2, ONLY the
                                                                       # two @pe/realtime-voice
                                                                       # TS2307 lines (pre-existing,
                                                                       # confirm against a plain
                                                                       # `git worktree add --detach
                                                                       # <tmp> e942446` if in doubt)
cd backend/relay-rs && cargo test --quiet barge_in_during_synthesis_delivers_no_audio_at_all
```

## Remaining gaps (unverified, stated honestly)

- The new `AppVoiceWiring.test.tsx` integration test was verified by code trace only, not by
  execution, because of the pre-existing `@pe/realtime-voice` worktree module-resolution gap. Once
  that gap is fixed (out of scope for A8 — it needs either a per-worktree symlink depth fix or an
  actual `npm install` of the registry packages), this test should be re-run to confirm it is
  actually green, not just plausible by trace.
- No physical-device AEC/speaker capture was run (codex's own `runtime.txt` already states this).
  The bounded `e2e:bargein` harness sample codex captured shows the fake-T0 provider completing
  synthesis before the delayed interrupt fires, so neither codex's run nor mine can claim a
  live-provider acoustic-cancellation number — only the deterministic control-path assertions above.
