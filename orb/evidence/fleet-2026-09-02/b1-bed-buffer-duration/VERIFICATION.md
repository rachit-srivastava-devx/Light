# B1 — bed buffer duration fix

Worktree: `company/products/adhd-focus-orb-worktrees/b1-bed-buffer-duration`
Branch: `fix/b1-bed-buffer-duration`, base `19a32af`
Repro: fresh clone/worktree of this repo at commit `19a32af`, apply this branch's commits, run the
commands below (see "environment setup" for the two symlinks a fresh worktree needs).

## What was found

`apps/mobile/src/presence/contracts.ts` defined two separate buffer-duration contracts:

- `BED_BUFFER_SECONDS = 30` — the real blueprint contract (blueprint 02 §2/§3: "30 s, not the
  guide's 2 s starter, so any residual periodicity is far below conscious detection over a 25-min
  session"), used only by `NoiseEngine.buildNoiseBuffer`'s default and by the audio-quality test
  suite (`NoiseEngine.test.ts`'s 30s/48kHz seam test).
- `BED_STARTUP_BUFFER_SECONDS = 0.25` — a second, much shorter constant, actually wired into
  production: `PresenceBoot.ts`'s `DEFAULT_AUDIO_GRAPH_CONFIG` used the startup constant, not the
  30s one, and `App.tsx`'s `getOrBootPresenceBed` call (the only production caller) boots that
  config permanently — the 0.25s buffer is never replaced, upgraded, or swapped for the 30s one
  anywhere in the codebase (confirmed by `grep -rn "upgrade\|swap\|replace"` across
  `apps/mobile/src/presence/` — the only hits are `ColorMorph.ts` comments about *not* needing a
  buffer swap for its brown↔pink morph, unrelated to this gap). `PresenceBoot.test.ts` asserted the
  0.25s buffer as the correct built shape, i.e. the defect was encoded as a passing requirement
  (same failure shape as the A9 entry in `FLEET-LEARNINGS.md`).

The 0.25s constant's own comment claimed justification: "the native app cannot spend seconds of JS
time before the bed starts." This is contradicted by the blueprint's own cost analysis: blueprint
02 §2 states generation cost for the **full 30s buffer** is "1.44 M samples × ≤ ~10 FLOPs ≈ < 15
MFLOP, once, at startup **(single-digit ms)**" — not seconds. The blueprint never describes a
fast-boot-then-upgrade/crossfade-swap pattern for this tradeoff (I grepped `upgrade`, `swap`,
`replace` in both `apps/mobile/src/presence/` and blueprint 02 itself — no such mechanism is
specified anywhere); the only crossfade in the design is the *intra-buffer* tail↔head loop seam
(§1/§2), which is orthogonal to buffer length. So the fix path is **not** "build the missing
upgrade mechanism" — it's "the premise for needing two buffers was false; use the one 30s buffer
directly, synchronously, since it is cheap enough."

## Fix

Removed the split: `PresenceBoot.ts`'s `DEFAULT_AUDIO_GRAPH_CONFIG.bed.source` now uses
`BED_BUFFER_SECONDS`/`BED_CROSSFADE_MS` (the real 30s/250ms contract) instead of the startup
constants. Deleted `BED_STARTUP_BUFFER_SECONDS`/`BED_STARTUP_CROSSFADE_MS` from `contracts.ts`
entirely (grepped whole repo afterward for `BED_STARTUP` — zero remaining references). This is the
"minimal correct fix" branch named in the brief: the 30s generation is cheap enough (single-digit
ms per the blueprint's own budget) to do synchronously at boot, so no upgrade/crossfade-swap
machinery needs to be built.

Files changed (see `diff.patch` in this directory for the full diff):
- `apps/mobile/src/presence/contracts.ts` — removed the two startup constants and their comment.
- `apps/mobile/src/presence/PresenceBoot.ts` — `DEFAULT_AUDIO_GRAPH_CONFIG` now points at the real
  30s/250ms contract constants.
- `apps/mobile/src/presence/PresenceBoot.test.ts` — updated the one assertion that encoded the
  0.25s buffer as correct behavior to assert the real 30s buffer instead (see RED/GREEN below).

## Test-first RED → GREEN

Modified `PresenceBoot.test.ts`'s existing `starts the looped bed source at app-open audio time`
test first (before touching implementation) to assert the buffer the *contract* requires
(`BED_BUFFER_SECONDS * BED_SAMPLE_RATE_HZ` samples) instead of the startup one, and ran it against
the still-unmodified implementation:

```
$ npx vitest run apps/mobile/src/presence/PresenceBoot.test.ts
 ❯ apps/mobile/src/presence/PresenceBoot.test.ts (4 tests | 1 failed)
   × bootPresenceBed > starts the looped bed source at app-open audio time
     → expected 12000 to be 1440000 // Object.is equality
 Test Files  1 failed (1)
      Tests  1 failed | 3 passed (4)
```

RED confirmed: 12000 samples = 0.25s @ 48kHz (the defect), expected 1,440,000 = 30s @ 48kHz (the
contract). Then implemented the fix (`PresenceBoot.ts` + `contracts.ts` above) and re-ran:

```
$ npx vitest run apps/mobile/src/presence/PresenceBoot.test.ts --testTimeout=20000
 ✓ apps/mobile/src/presence/PresenceBoot.test.ts (4 tests)
   ✓ bootPresenceBed > starts the looped bed source at app-open audio time
   ✓ bootPresenceBed > boots one graph for an app runtime across rerenders
   ✓ bootPresenceBed > does not cold-boot audio when turn control only asks for an existing bed
   ✓ bootPresenceBed > exposes explicit pause, recover, and terminal shutdown controls
 Test Files  1 passed (1)
      Tests  4 passed (4)
```

GREEN confirmed.

## Full presence + NoiseEngine suite

```
$ npx vitest run apps/mobile/src/presence --testTimeout=60000
 Test Files  9 passed (9)
      Tests  71 passed (71)
```

All 71 tests across `ColorMorph`, `PresenceEventQueue`, `InterruptionHandler`,
`StimulationController`, `AudioGraph`, `PresenceBoot`, `PresenceLifecycle`, `DuckingMixer`, and
`NoiseEngine` pass, including `NoiseEngine.test.ts`'s 30s/48kHz seam test
(`loops without an audible seam for both colours, at the real 30s/48kHz size`), which now runs the
same-shape 30s buffer PresenceBoot boots in production.

**Flake note (machine load, not this fix):** `NoiseEngine.test.ts`'s
`brownNoise > never exceeds the [-1, 1] clamp` test (a pure 50k-sample tight loop, no relation to
buffer-duration constants) timed out under concurrent multi-agent load on this shared machine
(`uptime` showed load averages up to 584 during this session — see `FLEET-LEARNINGS.md`'s
"System resource limit" entry) at both a 5s and a 20s default timeout in a couple of runs. Isolated
runs (`npx vitest run apps/mobile/src/presence/NoiseEngine.test.ts -t "never exceeds"
--testTimeout=60000`) pass reliably in ~13–21s. Not related to this change — it is a raw JS loop
timing test, present before and after the fix, and never touches `BED_BUFFER_SECONDS`/
`BED_STARTUP_*`.

## npm run verify (full gate)

```
$ npm run lint
boundary-lint: clean

$ npm run typecheck
apps/mobile/src/AppAudioBedFallback.test.tsx(171,9): error TS2345: Argument of type 'AtomizerPort'
is not assignable to parameter of type 'number'.
```

`typecheck` exits non-zero, but this is a **pre-existing, unrelated** failure — confirmed by
isolating the baseline in a disposable detached worktree (never `git stash`, per
`FLEET-LEARNINGS.md`'s hard boundary on the shared stash stack):

```
$ git worktree add --detach <scratch> 19a32af   # base commit, before this fix
$ cd <scratch> && ln -s <sibling-checkout>/node_modules node_modules
$ npm run typecheck
apps/mobile/src/AppAudioBedFallback.test.tsx(171,9): error TS2345: Argument of type 'AtomizerPort'
is not assignable to parameter of type 'number'.
```

Byte-identical single error at the unmodified base commit — not caused by this branch.

```
$ npx vitest run --testTimeout=30000     # whole repo
 Test Files  3 failed | 54 passed (57)
      Tests  1 failed | 704 passed (705)
```

The 3 failing test files:
- `backend/gateway-sidecar/__tests__/adapterTelemetry.test.ts` and `.../complete.test.ts` — both
  fail on `Cannot find module '@pe/llm-gateway/adapters/memory'`. Pre-existing registry-resolution
  gap unrelated to the presence plane, matching the same `@pe/llm-gateway` module-resolution issue
  multiple other 2026-09-02 `FLEET-LEARNINGS.md` entries already document (A1, A9, ws-reconnect-
  timeout, tts-true-streaming, barge-in-wiring). Not touched by this task.
- `apps/mobile/src/presence/NoiseEngine.test.ts` — the single `brownNoise` clamp-test timeout flake
  described above (machine load; passes cleanly in isolation).

```
$ npm run test:rs
running 64 tests
test result: ok. 64 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.69s

$ npm run test:py
395 passed, 3 warnings in 72.00s
```

Rust and Python suites fully green, untouched by this change.

**`npm run verify` exit code: non-zero (2)**, entirely attributable to the two pre-existing gaps
above (`typecheck`'s `AtomizerPort` mismatch, and `test`'s `@pe/llm-gateway` resolution gap), both
confirmed present at unmodified base `19a32af` and unrelated to the bed-buffer-duration fix. Every
gate step that touches the presence plane (this task's actual scope) is green: lint clean, the
presence+NoiseEngine suite 71/71, `test:rs` 64/64, `test:py` 395/395.

## Environment setup for a fresh worktree (per FLEET-LEARNINGS.md gotchas)

A fresh `git worktree add` has no `node_modules` or Python `.venv` of its own (both gitignored):

```
ln -s ../../adhd-focus-orb/node_modules node_modules
ln -s ../../../../adhd-focus-orb/backend/relay-py/.venv backend/relay-py/.venv
```

(mind the relative depth — get it wrong and `tsc`/`vitest` report a different, confusing error set
rather than failing to resolve at all).

## Honest gaps

- **No audible-periodicity verification on real audio hardware.** The fix makes production use the
  same 30s/250ms buffer the `NoiseEngine.test.ts` seam/spectrum tests already validate
  numerically (amplitude+spectrum continuity at the wrap, brown vs pink energy distinction), but I
  did not — and could not, in this environment — listen to the actual looped bed on a device or
  verify absence of audible periodicity by ear/FFT-over-real-output. That evidence class is
  explicitly out of reach without a live device (same gap other 2026-09-02 entries in
  `FLEET-LEARNINGS.md` note for their own audio claims).
- **Boot-latency budget (bed audible within 120ms) not re-measured on device.** The blueprint's own
  cost estimate (single-digit ms to generate 30s @ 48kHz) leaves comfortable headroom under the
  120ms acceptance anchor, and no automated test in this repo asserts a wall-clock boot-time budget
  for `bootPresenceBed`/`getOrBootPresenceBed` (confirmed by grep — no test file references
  `boot_ms` or a timing assertion around this call), so there was no existing automated gate to
  regress. This is a genuine gap for a device-level re-verification, not a claim I'm making as
  already proven.
- **`npm run verify`'s two pre-existing failures are unresolved**, per scope (this task owns the
  presence plane's bed-buffer-duration invariant only; both gaps are documented and cross-referenced
  by multiple other in-flight fleet tasks this session, not something to fix opportunistically here
  per the "no drive-by refactors outside owned files" rule).
