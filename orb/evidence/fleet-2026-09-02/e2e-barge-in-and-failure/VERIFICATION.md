# e2e-barge-in-and-failure — evidence

Task: end-to-end tests for the two FAILURE/EDGE-CASE paths fixed this session:
1. A8 barge-in cancels an in-flight provider call, fresh turn begins cleanly afterward.
2. A4 provider failure preserves presence (session stays open, audio-bed fallback fires,
   presence is never cancelled).

Drives a real compiled `orb-relay-rs` binary (release build) + a real HTTP "provider stub"
process over a real WebSocket — no vitest-level mocks — following the pattern already
established in this worktree by `e2e-human-simulator/barge_in_drive.mjs` /
`synthetic_mic.mjs` / `conversation_drive.mjs` (global WebSocket-style client, real relay
process, poll-with-timeout waits).

## What was already there vs. what this session finished

A prior worker (per the dispatch brief: got partway through, stuck waiting on a background
process) left real, uncommitted work in this worktree:
- `e2e-human-simulator/barge_in_cancels_call_drive.mjs` (A8 drive) — fully designed: spawns
  the real relay binary against a real HTTP provider-stub double, sends `speak` then
  `barge_in` mid-stream, asserts the provider's own HTTP connection was closed early
  (`closedEarly`, partial chunk count), cross-checks the relay's own devlog
  `latency.barge_in_yield` event, then proves a second `speak` on the same session completes
  uncancelled.
- `e2e-human-simulator/provider_failure_presence_drive.mjs` (A4 drive) — fully designed: arms
  the provider stub to fail the next TTS call, asserts `audio_bed_fallback` arrives, the
  socket stays open, a `Speak` sent while degraded is silently ignored, `Pause` is still the
  one way out (`closing`/`user_pause`), and statically greps the real
  `apps/mobile/src/App.tsx` `onAudioBedFallback` handler body for `cancelBy(`/`stopCapture(`/
  `.close(` calls it must not contain.
- `e2e-human-simulator/provider_stub.mjs` — a real HTTP server speaking relay-rs's documented
  `HttpContractProvider` wire contract (`/v1/stt/push`, `/v1/stt/end`, `/v1/tts/synthesize`
  chunked), with a slow-stream mode (for barge-in) and a fail-on-command mode (for A4),
  recording each TTS call's `closedEarly`/`chunksWrittenBeforeClose`.
- `package.json` — two new npm scripts wired (`e2e:bargein:cancel`, `e2e:provider-failure`).

This design was sound and needed no rework. What was NOT done: the prior worker never
actually got these two scripts to a real passing run — both had real bugs that only surface
when executed against the real binary (see below). This session ran them, found the bugs,
fixed them, and confirmed both pass repeatably.

## Bugs found and fixed (real defects in the harness, not the product)

1. **`findRelayBinary()` used `new URL(...).pathname` instead of `fileURLToPath`, in both
   drive files.** This repo's own absolute path contains a space
   (`.../Principal Engineering/...`), and `URL.pathname` leaves that percent-encoded
   (`%20`), so `existsSync()` on the prebuilt release binary always returned false and both
   scripts silently fell back to `cargo run --release`. Fixed by importing `fileURLToPath`
   from `node:url` and using it in both `barge_in_cancels_call_drive.mjs` and
   `provider_failure_presence_drive.mjs` (also fixed the same pattern in
   `provider_failure_presence_drive.mjs`'s `extractOnAudioBedFallbackHandler()` path to
   `apps/mobile/src/App.tsx`).

2. **`createInbox()`'s `next()` left a timed-out waiter in the `waiters` array forever, in
   both drive files.** When a call like `waitForFrameType(inbox, 'speech_starting', 1200)`
   times out as *expected* (no such frame should ever arrive) and the caller catches that
   error, the waiter object that was pushed for it is never removed. The next real message
   that arrives gets `waiters.shift()`'d onto that stale, already-rejected waiter — calling
   `resolve()` on an already-settled promise is a silent no-op — so the message is dropped on
   the floor instead of reaching whichever call is actually still waiting. This exact bug
   fired in `provider_failure_presence_drive.mjs`: the "voice work is refused while degraded"
   check's expected 1200ms timeout swallowed the real `closing` frame that arrived right after
   `Pause` was sent, so `waitForFrameType(inbox, 'closing', 4000)` timed out even though the
   relay had already sent `closing` correctly (confirmed independently with a raw
   `socket.on('message', ...)` listener showing the frame did arrive on the wire). Fixed by
   having each waiter remove itself from the array when its own timeout fires, in both files.

3. **A false-positive assertion in `provider_failure_presence_drive.mjs`.** The check "handler
   does NOT close the transport or stop capture" ran `/stopCapture\s*\(|\.close\s*\(/` against
   the raw extracted handler body, which includes the handler's own doc comment — a comment
   that explicitly documents the absence, in prose: `"no stopCapture(), no transport close"`.
   The regex matched `stopCapture(` inside that comment and reported a false FAIL even though
   the actual code has no such call. Fixed by stripping `//` line comments
   (`handlerBody.replace(/\/\/.*$/gm, '')`) before running both content checks.

None of these three fixes touch `backend/relay-rs`, `apps/mobile/src/App.tsx`, or any other
product file — they are all inside the three new, previously-uncommitted e2e harness files.

## Commands run and real output

### Preflight

```
$ date && stat -f "%Sm" backend/relay-rs/target/release/orb-relay-rs && \
  find backend/relay-rs/src -name "*.rs" -newer backend/relay-rs/target/release/orb-relay-rs
Wed Sep  2 23:25:29 IST 2026
Sep  2 23:21:59 2026
(no output — no .rs file is newer than the binary; the prebuilt release binary is current)
```

`node_modules` is the pre-existing symlink to `../../adhd-focus-orb/node_modules` (contains
`ws`, used directly by both drive scripts). No `.venv` was needed — neither scenario touches
Python.

### Scenario 1 — A8 barge-in cancels the in-flight provider call

```
$ node e2e-human-simulator/barge_in_cancels_call_drive.mjs
```
Exit code: **0**

```
[stub] provider double listening on http://127.0.0.1:56148
[relay-rs] starting via /Users/rachitsrivastava/youtube/Principal Engineering/company/products/adhd-focus-orb-worktrees/e2e-barge-in-and-failure/backend/relay-rs/target/release/orb-relay-rs
[relay-rs] orb-relay-rs listening on 127.0.0.1:18191
[relay-rs] accepted realtime connection with provider=http-contract
[test] connected to ws://127.0.0.1:18191, session=barge-in-cancel-1788372486312
[test] audio frames received before barge-in: 2
PASS  barge-in yields the floor (speech_complete observed)
PASS  exactly one synthesize call was made for turn 1 — stub recorded 1 call(s)
PASS  the provider HTTP connection was closed by the relay before the stub finished streaming — closedEarly=true, wrote 3/40 chunks
PASS  the call was interrupted mid-stream, not right at the start or right at the end — 3/40 chunks written
PASS  relay-rs devlog recorded the barge-in yield (server-side proof, not client-reported) — yield_latency_ms=21.7
PASS  the fresh turn reaches speech_complete cleanly
PASS  a second synthesize call was made for the fresh turn — stub recorded 2 call(s) total
PASS  the fresh turn ran to completion, uncancelled (proves the session was not left wedged) — closedEarly=false, wrote 40/40 chunks

ALL CHECKS PASSED
```

Full log saved verbatim: `barge_in_cancels_call_drive.log` (this directory). Re-run twice
more for stability (not a fluke): both exited 0, `ALL CHECKS PASSED` both times.

### Scenario 2 — A4 provider failure preserves presence

```
$ node e2e-human-simulator/provider_failure_presence_drive.mjs
```
Exit code: **0**

```
[stub] provider double listening on http://127.0.0.1:56161
[relay-rs] starting via /Users/rachitsrivastava/youtube/Principal Engineering/company/products/adhd-focus-orb-worktrees/e2e-barge-in-and-failure/backend/relay-rs/target/release/orb-relay-rs
[relay-rs] orb-relay-rs listening on 127.0.0.1:18192
[relay-rs] accepted realtime connection with provider=http-contract
[test] connected to ws://127.0.0.1:18192, session=provider-failure-presence-1788372494389
[test] provider stub armed to fail the next TTS call
[relay-rs] session provider-failure-presence-1788372494389: provider unavailable (degraded): provider returned non-200 response: HTTP/1.1 500 Internal Server Error
PASS  audio_bed_fallback frame observed (not silence, not a generic error)
PASS  the WebSocket is still open after the provider failure (readyState OPEN) — readyState=1
PASS  no close frame was received — not closed
PASS  voice work is refused while degraded (no speech_starting for a Speak sent after failure)
PASS  an explicit Pause still closes the session cleanly from Degraded — reason=user_pause
PASS  apps/mobile/src/App.tsx onAudioBedFallback handler does NOT call cancelBy(...) (presence preserved) — handler body is 1169 chars, no cancelBy call
PASS  apps/mobile/src/App.tsx onAudioBedFallback handler does NOT close the transport or stop capture — no transport-teardown call found

ALL CHECKS PASSED
```

Full log saved verbatim: `provider_failure_presence_drive.log` (this directory). Re-run twice
more for stability: both exited 0, `ALL CHECKS PASSED` both times.

## Reproduce from a fresh clone

1. `git worktree add <path> e2e/e2e-barge-in-and-failure` (or check out this branch).
2. Symlink deps from the sibling checkout (this worktree does not have its own installed
   copies, per this session's established pattern —
   FLEET-LEARNINGS.md "Gotcha 3"/"repo-hygiene" entries):
   ```
   ln -s ../../adhd-focus-orb/node_modules node_modules
   ```
   (Python `.venv` is not needed for these two scripts.)
3. Build the release relay binary if `backend/relay-rs/target/release/orb-relay-rs` does not
   already exist or is stale relative to `backend/relay-rs/src/**/*.rs`:
   ```
   cd backend/relay-rs && cargo build --release && cd ../..
   ```
4. Run both drives (each spawns its own relay-rs instance and provider-stub double on fixed
   ports 18191/18192, and tears both down in its `finally` block — no external dev-stack
   process needs to be running first):
   ```
   node e2e-human-simulator/barge_in_cancels_call_drive.mjs
   node e2e-human-simulator/provider_failure_presence_drive.mjs
   ```
   Or via the npm scripts wired into `package.json`:
   ```
   npm run e2e:bargein:cancel
   npm run e2e:provider-failure
   ```
   Exit 0 and `ALL CHECKS PASSED` on both for success; exit 6 lists which named check(s)
   failed.

## Honest gaps

- Neither script drives the real mobile client or a phone/simulator microphone/AEC path. The
  "user started speaking" trigger in scenario 1 is a directly-sent `barge_in` control frame,
  not a real 200ms-sustained on-device VAD detection through
  `apps/mobile/src/voice/BargeIn.ts`'s `createBargeInCoordinator` — that path is already
  covered in isolation by `apps/mobile/src/voice/BargeInPlaybackLatency.test.ts` (per this
  file's own header note); this script covers the transport/provider-cancellation contract
  that call rides on, against a real relay-rs binary, not the on-device detector itself.
- Scenario 2's client-side property ("the handler never cancels presence") is checked
  statically by reading `apps/mobile/src/App.tsx`'s real `onAudioBedFallback` source — the
  same technique the earlier verifier used to originally catch the client-wiring gap
  (FLEET-LEARNINGS.md, "client-wiring gap for A4 audio_bed_fallback" entry) — not by mounting
  a live RN app and observing presence state dynamically. No simulator/device run was done as
  part of this task.
- Did not run the full `npm run verify`/whole-repo `vitest`/`cargo test` gates as part of this
  task — out of scope for "write and run these two e2e drives," and this session's own
  FLEET-LEARNINGS.md already documents (repeatedly, across many entries dated 2026-09-02) a
  pre-existing, unrelated `@pe/realtime-voice`/`@pe/llm-gateway` module-resolution typecheck
  break at this worktree depth, confirmed independently by multiple other workers via
  disposable-worktree isolation against unmodified `main`/`e942446`. Not re-verified here
  since this task's own diff touches none of those files.
- `backend/relay-rs/target/release/orb-relay-rs` is a prebuilt binary already present in this
  worktree at task start (built 2026-09-02 23:21:59, after all 8 merged voice-pipeline defect
  fixes per the dispatch brief, and confirmed no `.rs` source file in `backend/relay-rs/src` is
  newer than it) — this task did not rebuild it, only ran it.
