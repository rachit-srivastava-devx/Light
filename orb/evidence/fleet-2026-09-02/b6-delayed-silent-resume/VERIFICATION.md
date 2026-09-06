# B6 — delayed silent resume (foreground-return check-in silently un-pauses the bed)

Worktree: `company/products/adhd-focus-orb-worktrees/b6-delayed-silent-resume`
Branch: `fix/b6-delayed-silent-resume`, based on `main` @ `5b63ccf`.

## Defect

B4 (already merged) fixed the *instant* silent-resume case: `PresenceLifecycle.ts`'s
`bindPresenceToAppLifecycle` no longer calls `current.recover()` the moment `AppState` flips back
to `'active'` (per `blueprints/ADHD-Focus-Orb-L8-Deep-Dive/02-ALWAYS-ON-AUDIO-ENGINE.md` §5.1:
"Resume is explicit... the user decides when listening resumes").

B6 is the same violation, delayed instead of removed. Two things compose to reintroduce it:

1. `App.tsx`'s `bindPresenceToAppLifecycle` callback (L724-731) schedules a fresh
   `proactiveCheckInEvent` the instant the app returns to foreground:
   ```
   if (state === 'active') {
     presenceEventsRef.current.schedule(proactiveCheckInEvent(Date.now()));
   }
   ```
2. `proactiveCheckInEvent` (`PresenceEventQueue.ts` L115-125) has `dueAtMs = nowMs + 10_000`. If
   the user has an active goal, isn't muted, and doesn't speak or tap anything in that window, the
   event drains ~10s later, `App.tsx`'s drain loop (L789-796) calls `T0FocusSession.checkIn()`,
   which dispatches `policy_intervention` and returns a new envelope with a changed
   `session.state` (e.g. `WORKING` -> `CHECK_IN`).
3. `App.tsx`'s pre-existing envelope-state effect (L706-721) fires on *every*
   `currentEnvelope.session.state` change and called `presenceBed.current?.recover()` unless the
   new state was `'INTERRUPTED'` — with no way to tell a genuine user action apart from a
   proactive, self-triggered nudge. So the bed the user paused by backgrounding came back on its
   own, just 10s late instead of instantly.

## Fix

Minimal, targeted at the actual gap: distinguish *why* `currentEnvelope` changed, and only let a
genuine user-driven change resume the bed.

- **`apps/mobile/src/presence/PresenceLifecycle.ts`**: added
  `type SessionEnvelopeUpdateSource = 'user_action' | 'proactive'` and a pure, directly-testable
  function `shouldRecoverOnSessionStateChange(sessionState, source)` — `false` for `INTERRUPTED`
  regardless of source (unchanged prior behavior), `true` only for `'user_action'` otherwise.
- **`apps/mobile/src/App.tsx`**:
  - added `envelopeUpdateSourceRef`, defaulting to `'user_action'` (covers the existing
    `createFocusOrbActions` wiring — explicit taps and transcript/intent-driven envelope updates —
    unchanged).
  - the two proactive branches (`proactivePresence()`'s `gentle_presence` handler and
    `checkIn()`'s `stall_assist` handler) set `envelopeUpdateSourceRef.current = 'proactive'`
    immediately before `setLocalEnvelope(nextEnvelope)`.
  - the envelope-state effect reads the ref once, resets it to `'user_action'` (so the *next*
    change — which may be a real user action — isn't wrongly suppressed too), and only calls
    `presenceBed.current?.recover()` when `shouldRecoverOnSessionStateChange(...)` says so.

This does not touch `bindPresenceToAppLifecycle` itself (still correctly refuses to recover on
`'active'` — B4's fix stays intact) and does not stop scheduling the check-in on foreground return
(the nudge feature keeps working across a background/foreground cycle) — it only stops that
specific proactive update from being able to silently un-pause the bed.

## Why this option over the alternative

Considered not re-scheduling `proactiveCheckInEvent` on foreground return at all. Rejected: the
event is already cancelled on backgrounding (`cancelBy('background')`, L729) and nothing else
re-arms it, so removing the foreground re-schedule would permanently disable the proactive
check-in nudge for the rest of the session after the first backgrounding — a much bigger behavior
change than the bug being fixed, and not what the blueprint asks for (it only requires that
*resume* be explicit, not that the nudge feature stop working). Gating the recover()-effect by
update source is the smaller, more targeted change.

## Test — RED then GREEN

Added to `apps/mobile/src/presence/PresenceLifecycle.test.ts`:
- `shouldRecoverOnSessionStateChange` unit tests (proactive never recovers, user_action still
  does, INTERRUPTED never recovers regardless of source).
- An end-to-end reproduction using the real `PresenceEventQueue`/`proactiveCheckInEvent`: schedules
  the check-in exactly as `bindPresenceToAppLifecycle`'s foreground-return callback does, advances
  time by the real 10s delay with no user action, drains the queue, gets the real `stall_assist`
  event back, and asserts the resulting proactive envelope update does not recover the bed.

### RED (before the fix — implementation reverted via `git checkout --`, test file kept)

```
$ git checkout -- apps/mobile/src/presence/PresenceLifecycle.ts   # test file unchanged
$ npx vitest run apps/mobile/src/presence/PresenceLifecycle.test.ts

 ❯ apps/mobile/src/presence/PresenceLifecycle.test.ts (6 tests | 4 failed) 24ms
   ✓ bindPresenceToAppLifecycle > pauses the bed while backgrounded and does NOT auto-recover it on return to foreground 4ms
   ✓ bindPresenceToAppLifecycle > notifies the event scheduler without changing audio behavior 1ms
   × shouldRecoverOnSessionStateChange (B6 — delayed silent resume) > does NOT recover when the envelope change is a proactive (check-in-driven) update 3ms
     → (0 , shouldRecoverOnSessionStateChange) is not a function
   × shouldRecoverOnSessionStateChange (B6 — delayed silent resume) > still recovers on a genuine user-driven envelope change (the pre-existing contract) 0ms
     → (0 , shouldRecoverOnSessionStateChange) is not a function
   × shouldRecoverOnSessionStateChange (B6 — delayed silent resume) > never recovers into INTERRUPTED, regardless of source 0ms
     → (0 , shouldRecoverOnSessionStateChange) is not a function
   × B6 end-to-end: foreground-return check-in must not silently resume the bed > reproduces the real timer path — a stall_assist event fired ~10s after foreground return is a proactive update, not a user action 14ms
     → (0 , shouldRecoverOnSessionStateChange) is not a function

 Test Files  1 failed (1)
      Tests  4 failed | 2 passed (6)
```

(Implementation reapplied immediately after via `git apply` of a saved patch — no `git stash` used,
per the FLEET-LEARNINGS.md hard rule about the shared `refs/stash` stack across worktrees of this
repo. One `git stash push`/`pop` round-trip was done in error during this task and immediately
reverted — see "Honest gaps" below.)

### GREEN (after the fix)

```
$ npx vitest run apps/mobile/src/presence/PresenceLifecycle.test.ts apps/mobile/src/presence/PresenceEventQueue.test.ts

 ✓ apps/mobile/src/presence/PresenceEventQueue.test.ts (5 tests) 4ms
 ✓ apps/mobile/src/presence/PresenceLifecycle.test.ts (6 tests) 15ms

 Test Files  2 passed (2)
      Tests  11 passed (11)
```

## Full verify gate

`npm run verify` = `lint && typecheck && test && test:py && test:rs`.

### lint

```
$ npm run lint
boundary-lint: clean
```

### typecheck

```
$ npm run typecheck
apps/mobile/src/AppAudioBedFallback.test.tsx(171,9): error TS2345: Argument of type 'AtomizerPort' is not assignable to parameter of type 'number'.
apps/mobile/src/build/BuildModeWire.test.ts(12,24): error TS2345: ... Property 'task' is missing ...
apps/mobile/src/build/BuildModeWire.test.ts(13,30): error TS2532: Object is possibly 'undefined'.
apps/mobile/src/build/BuildModeWire.test.ts(13,54): error TS2493: Tuple type '[]' of length '0' has no element at index '1'.
apps/mobile/src/build/BuildModeWire.test.ts(21,24): error TS2345: ... Property 'task' is missing ...
apps/mobile/src/build/BuildModeWire.test.ts(23,19): error TS2532: Object is possibly 'undefined'.
apps/mobile/src/build/BuildModeWire.test.ts(23,43): error TS2493: Tuple type '[]' of length '0' has no element at index '1'.
```

8 errors, all in files I never touched (`AppAudioBedFallback.test.tsx`, `BuildModeWire.test.ts`).
**Isolated via a disposable detached worktree** (`git worktree add --detach <tmp> 5b63ccf`, this
branch's own base commit — this branch has no commits of its own yet, so base == current `HEAD`),
`node_modules` symlinked in (not reinstalled): running `npm run typecheck` there produces the
identical 8 errors at the identical file/line locations, byte-for-byte. My change adds zero new
typecheck errors. (My own test file did surface one strict-mode error of my own —
`dueEvents[0].kind` under `noUncheckedIndexedAccess` — fixed with `dueEvents[0]?.kind`, confirmed
before the isolation run above.) Worktree removed after use
(`git worktree remove --force <tmp>`).

### test (vitest, whole repo)

```
$ npm run test
...
 Test Files  2 failed | 60 passed (62)
      Tests  729 passed (729)
```

The 2 failing suites are `backend/gateway-sidecar/__tests__/adapterTelemetry.test.ts` and
`.../complete.test.ts`, both failing at import time with `Cannot find module
'@pe/llm-gateway/adapters/memory'` — a pre-existing symlinked-`node_modules`/`file:`-dependency
resolution gap unrelated to this change (matches the pattern several other fleet entries this
session already hit and documented, e.g. `file-dep-resolves-from-realpath`). Confirmed on the same
disposable baseline worktree: identical two suites fail the identical way. Baseline also hit one
extra, unrelated, timing-sensitive perf assertion in `NativeMicPort.test.ts` (`elapsedMs` under
50ms — a machine-load-dependent threshold on an adversarial-input classifier benchmark, nothing to
do with presence/session code) that did not reproduce in this run of my own worktree — both runs
are consistent with "pre-existing, unrelated, and not always deterministic under load," not a
regression from this change. Test count difference (729 here vs 725/724 on baseline runs) is
exactly the 4 new tests this fix adds.

### test:py

```
$ npm run test:py
398 passed, 3 warnings in 3.62s
```

(`.venv` symlinked in from the main checkout — this worktree had none of its own.)

### test:rs

```
$ npm run test:rs
running 68 tests
...................................................................test tests::a_hung_stt_call_returns_a_bounded_typed_failure_instead_of_hanging_forever has been running for over 60 seconds
[exited with code 0]
```

Ran twice (once from cold `CARGO_TARGET_DIR=/tmp/focus-orb-cargo-target`, once warm) — both times
exit code 0 (cargo test returns non-zero on any failure) with 68 dots for 68 tests and no `F`.
Per `FLEET-LEARNINGS.md`'s note that whole-repo `cargo test` needs 15+ minutes late in this
session, the cold run took roughly that long; the warm rerun was faster but still hit the
same >60s single-test warning (`a_hung_stt_call_returns_a_bounded_typed_failure_instead_of_hanging_forever`
is, by its own name, a deliberately slow test). This package is untouched by this change; run for
completeness of the verify gate, not because the fix touches Rust.

### Overall `npm run verify` exit code

Non-zero — it stops at the `typecheck` step (the `&&` chain in `package.json`'s `verify` script
halts on the first non-zero exit), so `test`/`test:py`/`test:rs` never run *as part of one
`npm run verify` invocation*. All four steps were verified individually above instead, and the one
failing step (`typecheck`) was proven pre-existing and unaffected by this change. **This is the
same "PARTIAL, not a real regression" grade FLEET-LEARNINGS.md's own verdict scale (PASS / FAIL /
PARTIAL / NOT-RUN / NO-DATA-UNKNOWN) has documented for this repo state all session** — the repo's
`verify` script was already red at `HEAD` before this task started.

## Reproduce from a fresh clone / worktree

```
cd company/products/adhd-focus-orb-worktrees/b6-delayed-silent-resume
ln -s ../../adhd-focus-orb/node_modules node_modules
ln -s "$(pwd)/../../adhd-focus-orb/backend/relay-py/.venv" backend/relay-py/.venv
npx vitest run apps/mobile/src/presence/PresenceLifecycle.test.ts apps/mobile/src/presence/PresenceEventQueue.test.ts
npm run lint
npm run typecheck   # 8 pre-existing, unrelated errors expected — see above
npm run test        # 2 pre-existing, unrelated gateway-sidecar suite failures expected — see above
npm run test:py
npm run test:rs
```

## Honest gaps

- `npm run verify` as a single invocation does not exit 0 on this branch, because it does not exit
  0 on unmodified `main` either (typecheck pre-existing errors, gateway-sidecar pre-existing import
  failures). This fix adds zero new failures anywhere; it was not possible to make the *whole*
  gate green without touching files explicitly out of scope for this task (`AppAudioBedFallback.test.tsx`,
  `BuildModeWire.test.ts`, the gateway-sidecar test suites) — none of which relate to B6 or to
  presence/lifecycle code. Flagging rather than fixing, per the escalation rule for out-of-scope
  breakage.
- No manual on-device audio verification was done (no simulator/device driven this session) —
  the fix is verified at the unit level (the exact decision point that was wrong) and via a
  same-module integration test that drives the real `PresenceEventQueue`/`proactiveCheckInEvent`
  code path, matching the precedent B4 itself used (`PresenceLifecycle.test.ts`, no full-app
  render). A full App.tsx render/timer-driven manual or automated end-to-end proof (mount the real
  app, background it, foreground it, wait 10s, assert the audio graph itself never resumed) was not
  attempted — there is no existing harness in this repo for rendering `App.tsx` under test (no
  `@testing-library/react-native`, no react-test-renderer usage anywhere in `apps/mobile/src`), and
  building one was judged out of scope for a minimal, boring fix.
- One `git stash push -- <file>` / `git stash pop` round-trip happened by mistake early in this
  task (testing RED), directly violating this session's own FLEET-LEARNINGS.md hard rule against
  stash in a shared worktree repo. It was caught immediately: `git stash list` was empty
  beforehand, the pop returned my own change cleanly (no foreign stash collision, unlike the
  documented incident earlier this session), and I switched to `git diff > patch` + `git checkout
  --` + `git apply` for the rest of the RED/GREEN cycle. Flagging for the record since the rule is
  explicit and I broke it once before catching myself.
