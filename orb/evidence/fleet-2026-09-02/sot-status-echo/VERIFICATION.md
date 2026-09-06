# Speed-of-Thought P0 — Unit U5 (status-echo) verification

Branch: `sot-p0/status-echo` (worktree `adhd-focus-orb-worktrees/sot-status-echo`)
Date: 2026-09-02
Head at verify time: `4574d9f` (fast-forwarded from base `e942446` to include U1 `lld-v1.ts` from main
`79652d9`). Note: `main` moved past this branch during the session (concurrent fleet workers); the
U5 diff is purely additive (3 new files).

## Ownership (contract §6 U5)

- `apps/mobile/src/build/BuildEnvelope.ts` (new)
- `apps/mobile/src/build/StatusEcho.ts` (new)
- `apps/mobile/src/build/StatusEcho.test.ts` (new — frozen from contract §6 U5, byte-unchanged)

Imports `DepthScore` type from U1's real `apps/mobile/src/build/lld-v1.ts` (merged to main at
`79652d9`), as instructed. No U3 `LldReadyGate.ts` file is depended on (U3 not merged; `GateVerdict`/
`GateReason` are structurally mirrored in `BuildEnvelope.ts`, union-shape matches U3 so the two
interlock via the shared frozen test).

Contracted test-file rule: `StatusEcho.test.ts` is byte-identical to contract §6 U5. `diff` against
the contract section is empty.

## RED (greenfield)

First run of the frozen test before `StatusEcho.ts`/`BuildEnvelope.ts` existed:

```
Can't find module ./StatusEcho imported from .../StatusEcho.test.ts
 ❯ apps/mobile/src/build/StatusEcho.test.ts:2:1
   import { statusEcho, echoSpeech } from './StatusEcho';

Test Files  1 failed (1)
      Tests  no tests
```

(RED log: `RED-focused.log`.)

## GREEN (focused)

```
 ✓ apps/mobile/src/build/StatusEcho.test.ts (3 tests) 9ms

 Test Files  1 passed (1)
      Tests  3 passed (3)
```

U5-T1 refusal echoes real check id (`R17-DERIV`) and `score.ratio` 0.643.
U5-T2 frozen echo carries `handoff_status: 'artifact_written_no_consumer'`.
U5-T3 no echo speech matches forbidden downstream vocabulary `/\b(PR|pull request|merged|merging|
built|building|deployed|shipped|attested)\b/i`.

## Verify gate (`npm run verify`)

Exits `2`, stopping at the `typecheck` step with exactly one error, isolated as pre-existing:

```
apps/mobile/src/AppAudioBedFallback.test.tsx(171,9): error TS2345:
  Argument of type 'AtomizerPort' is not assignable to parameter of type 'number'.
```

**Isolation (disposable detached worktree, no `git stash`):** the identical error reproduces on
pristine `main` (`5a4dee5d`) with no U5 files present (worktree `ln -s` to the shared sibling
`node_modules`, same `npx tsc --noEmit -p apps/mobile`). U5 adds zero typecheck errors.

Steps individually:
- `npm run lint` (boundary-lint): **clean** (0 errors).
- `npx tsc --noEmit -p apps/mobile`: only the pre-existing `AppAudioBedFallback.test.tsx:171` error.
- `npx vitest run apps/mobile/src/build/StatusEcho.test.ts`: **3/3 passed**.
- Whole `apps/mobile` vitest: 596 passed / 4 failed — the 4 failures are pre-existing, in suites U5
  does not touch and that fail identically on pristine main via the disposable worktree:
  - `NoiseEngine.test.ts` (3: white-noise amplitude, brown-noise clamp, buildNoiseBuffer seam) — 2 failed
    even in isolation, reproduced on pristine main `5a4dee5d`.
  - `AppVoiceWiring.test.tsx` (1: "reaches end_of_turn ... earlier than the VAD-only path") — 1 failed,
    reproduced on pristine main `5a4dee5d`.
  - (`NativeMicPort.test.ts` flaked earlier in a concurrent-load run; learnings already flag
    NoiseEngine/NativeMicPort as load-sensitive.)

## Evidence files

- `GREEN-focused.log` — focused vitest, 3/3.
- `RED-focused.log` — greenfield missing-module failure.
- `TYPECHECK-mobile-full.log` — full apps/mobile tsc (single pre-existing error).
- `LINT.log` — boundary-lint clean.
- `VERIFY-full.log` — `npm run verify` full output (exits 2 at typecheck).

## Not verified (honest gaps)

- `npm run verify` full chain (pytest + cargo) never ran to completion: the gate halts at the
  pre-existing `typecheck` error, so `test:py`/`test:rs` were not reached through `verify`.
- No on-device audio / no audible speech: U5 is a pure text formatter; its `echoSpeech` output was
  asserted against the denylist only (U5-T3). No device build was exercised.
- `BuildEnvelope.ts`'s `BuildResponseEnvelope` (the envelope superset) has no runtime consumer in P0
  (U2/U4 wire the build FSM/mode separately); it is the contract §5 shape, exported for composition.
