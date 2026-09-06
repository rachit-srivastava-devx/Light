# B2 — color-morph timing (`presence/ColorMorph.ts` / `App.tsx:712`)

Worktree: `company/products/adhd-focus-orb-worktrees/b2-color-morph-timing`
Branch: `fix/b2-color-morph-timing`, base `19a32af` (this worktree's own HEAD at start, current `main`).

## Defect confirmed

`apps/mobile/src/App.tsx`'s own `useEffect` (was at line 712) called:

```ts
presenceBed.current?.morphColor(currentEnvelope.session.state === 'STEP_PRESENT' ? 'thinking' : 'idle');
```

Cross-checked against three sources:

1. **`apps/mobile/src/AppModel.ts`'s own `voiceState()`** (line 154): `STEP_PRESENT` maps to
   `'speaking'` — the orb speaking an *already-computed* step, not the crew computing one.
2. **`apps/mobile/src/session/contracts.ts`** (the frozen 9-state FSM vocabulary): there is no
   dedicated "atomizer is computing" state. `atomize_ready` fires the moment the atomizer/context-pack
   retrieval *finishes*, transitioning straight from INTAKE/CLARIFY to STEP_PRESENT. The computation
   itself happens as an in-flight `await` inside `T0FocusSession.ts`'s `atomizeTask()`, invisible to the
   session-state enum entirely — confirming the defect-hunt's note that "there may be no dedicated
   state for this."
3. **`apps/mobile/src/presence/ColorMorph.ts`'s own header** (quoting blueprint
   `02-ALWAYS-ON-AUDIO-ENGINE.md` §7's table verbatim): "Thinking (crew forming steps) | morph toward
   pink" is a distinct row from "Step ready / win | brief swell + chime" — and the header is explicit
   that modelling step-ready as a color change "would leave the bed permanently brighter after a step
   is presented, contradicting §7." The pre-fix code did exactly that: STEP_PRESENT (the
   step-just-became-ready/speaking moment) drove the bed to 'thinking' (pink), and per
   `T0FocusSession.ts`'s `speakCurrentStep()` dispatching `spoken_complete` synchronously, that pink
   state would then persist for the entire step-presented/speaking window before reverting.

No ADR documents this as an accepted simplification (checked `docs/adr/*.md` for any color-morph /
STEP_PRESENT / atomizer discussion — none found). This is a confirmed bug, not a documented gap.

## The correct signal already exists — no new state needed

`apps/mobile/src/runtime/VoiceLoopController.ts` already owns a **second, correct** color-morph
driver: `schedulePresenceMorph(status)` (private helper, calls `graph.morphColor(status)` directly on
the same presence-bed graph). It is wired to the real computation window:

- `completeTurn()` / `signalTurnEnd()` (turn-end signaled, i.e. the user just finished speaking and the
  crew is about to compute a response) -> `schedulePresenceMorph('thinking')`.
- `handleAudioFrame`'s `start_listening` / `voice_active` branches (the user has started speaking
  again) -> `schedulePresenceMorph('idle')`.

This is exactly "the correct existing state/signal" the brief asked me to check for — it just wasn't
being used by `App.tsx`'s own, separate, incorrect driver, which fought it by re-deciding the bed color
from session-envelope state on every render.

## Fix

Removed `App.tsx`'s duplicate/incorrect `morphColor(...)` call (kept `recover()`/`pause()`, which are
the unrelated bed-lifecycle-interruption logic in the same effect). Color-morph is now owned solely by
`VoiceLoopController.ts`'s `schedulePresenceMorph`, which already implements the correct window. See
`App.tsx.diff` in this directory for the exact change.

## Test — written first, confirmed RED, then GREEN

New file: `apps/mobile/src/AppColorMorphTiming.test.tsx` (copy in this directory). Mounts the real
`App.tsx` (same harness pattern as `AppVoiceWiring.test.tsx`) with a fake `AudioContext` that records
every `BiquadFilterNode` it creates, and uses `App.tsx`'s own `envelope` test-injection prop (already
present in `FocusOrbAppProps` for exactly this kind of isolated assertion) to move the session state to
`STEP_PRESENT` *without* driving the mic/relay/VoiceLoopController at all — isolating the App.tsx-owned
driver from VoiceLoopController's (never exercised by this test, so its calls cannot make the test pass
for the wrong reason). Asserts the bed's lowpass cutoff never reaches `BED_STATUS_CUTOFF_HZ.thinking`
(1200 Hz) as a result.

- **RED** (against unmodified `apps/mobile/src/App.tsx` — reconstructed via `git show HEAD:...`, then
  restored): `red.txt` -- `expected 1200 not to be 1200`, i.e. the bed morphed to 'thinking' purely from
  the STEP_PRESENT envelope, confirming the defect.
- **GREEN** (after the fix): `green.txt` -- 1/1 passed.

## Full verification

- `npm run lint`: clean (`lint.txt`).
- `npm run typecheck`: exits 2, but the ONLY error is `apps/mobile/src/AppAudioBedFallback.test.tsx(171,9):
  error TS2345: Argument of type 'AtomizerPort' is not assignable to parameter of type 'number'.`
  (`typecheck.txt`). Confirmed **pre-existing**: reconstructed `App.tsx` to unmodified `HEAD` (the only
  file this task touches besides the new test) and rechecked -- identical single error, same line. This
  task's own new test file (`AppColorMorphTiming.test.tsx`) typechecks clean.
- `npx vitest run` (whole repo): **706/706 tests passed**. 2 suites fail to *load*
  (`backend/gateway-sidecar/__tests__/adapterTelemetry.test.ts`,
  `.../complete.test.ts`) -- `Cannot find module '@pe/llm-gateway/adapters/memory'`. Confirmed
  **pre-existing**: reproduced identically in a disposable `git worktree add --detach /tmp/b2-baseline-check
  HEAD` of the unmodified base (symlinked this worktree's `node_modules` in, per the session's own
  documented pattern for this worktree-depth issue) -- same error, same two files, before touching
  anything (`full-vitest.txt`; no `git stash` used, per the shared-`refs/stash` hazard other entries in
  `FLEET-LEARNINGS.md` already hit this session).
- `npm run test:py`: 395 passed (`test-py.txt`; backend/relay-py untouched by this task -- symlinked the
  sibling checkout's `.venv` in, since this worktree had none, matching the pattern the A9 entry in
  `FLEET-LEARNINGS.md` used).
- `npm run test:rs`: 64 passed (`test-rs.txt`; `backend/relay-rs` untouched by this task).

`npm run verify` itself exits non-zero purely because of the pre-existing `typecheck` failure above
(halts before reaching `backend/gateway-sidecar`/`backend/voice-provider-sidecar`, and before `test`);
every step run individually and in isolation is either green or the identical pre-existing failure.

## Reproduce from a fresh clone

```bash
cd company/products/adhd-focus-orb-worktrees/b2-color-morph-timing   # or any fresh checkout of this branch
ln -s ../../adhd-focus-orb/node_modules node_modules                  # sibling checkout's deps (this
                                                                        # worktree sits one level deeper,
                                                                        # per prior FLEET-LEARNINGS entries)
ln -s ../../../../adhd-focus-orb/backend/relay-py/.venv backend/relay-py/.venv  # only needed for test:py

npx vitest run apps/mobile/src/AppColorMorphTiming.test.tsx   # 1 passed
npm run lint                                                  # clean
npm run typecheck                                             # exits 2 on the pre-existing, unrelated
                                                                # AppAudioBedFallback.test.tsx error
npx vitest run                                                 # 706/706 tests passed, 2 suites fail to
                                                                # load on the pre-existing @pe/llm-gateway gap
npm run test:py                                                # 395 passed
npm run test:rs                                                # 64 passed
```

## Files changed

- `apps/mobile/src/App.tsx` -- removed the incorrect `morphColor(...)` call from the session-state
  effect; kept `recover()`/`pause()`.
- `apps/mobile/src/AppColorMorphTiming.test.tsx` -- new, RED->GREEN acceptance test for this fix.

## Gaps / honest notes

- No physical-device or real audio playback verification was done (no live provider/device in this
  environment); the fix and test operate on the Web Audio graph's scheduled parameter values, not
  audible sound.
- `VoiceLoopController.ts`'s own `schedulePresenceMorph` leaves the bed pink from turn-end-signaled
  through the *entire* subsequent step-presented/speaking window, only reverting to idle once the user
  starts speaking again (on `start_listening`/`voice_active`) -- it never explicitly reverts to 'idle'
  the moment the crew's response is actually ready (before speaking it). This may itself be a narrower
  timing gap against blueprint 02 §7's exact ~400ms "thinking" window, but it is a **pre-existing**
  design in a file this task did not need to change to fix the flagged defect (App.tsx's own duplicate,
  wrongly-timed driver), and is out of this task's scope. Flagging it for a separate look rather than
  silently expanding scope.
