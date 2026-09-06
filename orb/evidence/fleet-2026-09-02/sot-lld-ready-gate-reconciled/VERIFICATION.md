# U3 lld-ready-gate — reconciliation against merged U1 (`lld-v1.ts`)

Branch `sot-p0/lld-ready-gate`, worktree
`company/products/adhd-focus-orb-worktrees/sot-lld-ready-gate`. This reconciles the U3 gate
(originally built against a local placeholder mirror because U1 hadn't merged yet) onto the real,
merged `lld.v1` schema, per `docs/SPEED-OF-THOUGHT-P0-CONTRACT.md`.

## 1. Rebase

`git fetch . main:refs/tmp-main-check` (local ref check) then `git rebase main` from inside the
worktree. Base was `e942446`; `main` had advanced to `012154d` (23 commits ahead, including the U1
merge at `79652d9`).

Conflicts (all expected, all resolved manually):

- **`.gitignore`** — both U1 and U3 independently added the identical 3-line
  `!apps/mobile/src/build/` un-ignore fix (different comment wording only). Took `main`'s wording
  verbatim, since the task explicitly called this the trivial case.
- **`apps/mobile/src/build/fixtures/AUTHORSHIP.md`** — add/add, both sides wrote a self-consistent
  authorship record. Resolved by taking U1's canonical version (see §2) and appending a
  "Reconciliation" section documenting this branch's re-verification.
- **`apps/mobile/src/build/fixtures/complete_module.json`** — add/add, U1 authored a schema-focused
  fixture (`node_id: "lld-v1-schema"`), U3 self-authored a gate-focused one
  (`node_id: "lld-ready-gate"`). Took U1's version (§2 explains why).
- **`apps/mobile/src/build/fixtures/one_line_freeze.json`** — same shape of conflict. Took U1's
  version.

Result: `git rebase --continue` succeeded, single commit `72a077b` (was `ff42d43` at base) on top of
`012154d`. `main` has since moved further ahead in a concurrent session (`4574d9f`, docs-only) but
that commit only touches `docs/`, not any file this unit owns — not re-rebased onto it since there
is nothing to reconcile.

## 2. Fixture reconciliation (contract §3.3, task step 3)

`docs/SPEED-OF-THOUGHT-P0-CONTRACT.md` §3.3 names U1 as the fixtures' canonical author ("U1 authors
fixtures; U3 authors the gate") specifically so the bad fixture is independently authored, not
self-authored by the gate's own builder. At the time U3 was originally built, U1 hadn't merged, so
U3 self-authored both fixtures and flagged them `self-authored/unverified` in `AUTHORSHIP.md`,
per the contract's own instruction not to assume an unverified fixture is correct.

This reconciliation pass took **U1's fixtures** (now the real, merged, independently-authored pair)
over U3's self-authored copies, and independently re-verified both against the merged
`LldReadyGate.ts` — by hand-tracing all 14 checks (documented in the updated `AUTHORSHIP.md`) and by
running the frozen `LldReadyGate.test.ts` suite against them:

- `complete_module.json` (U1, `node_id: "lld-v1-schema"`): passes all 14 named checks
  (`C1-OPEN` through `SHAPE`) against the frozen test's `REFS`
  (`owners: ['rachit@devxlabs.ai']` — matches `contracts/owners.v1.json`;
  `registry_paths: ['registry/services/llm-gateway', 'registry/features/cost-control-plane']`;
  `known_node_ids: ['orb-freeze-ledger']`, satisfied by the fixture's `deps: ["orb-freeze-ledger"]`).
  `lldReady` returns `READY` with `checked === GATE_CHECK_IDS.length === 14` (U3-T2).
- `one_line_freeze.json` (U1, `node_id: "x"`): trips 10 distinct check ids against the merged gate
  (`SHAPE`, `C1-OPEN`, `C2-OWNER`, `C3-ACC-GROUND`, `C3-ACC-NONTAUT`, `R17-DERIV`, `R21-ALTS`,
  `R21-FAIL`, `REG-VERDICT`, `IFACE`) — well over the contract's ≥5 floor (U3-T1).

Both are confirmed by the actual test run in §4 below (`LldReadyGate.test.ts`: 6/6 pass, including
U3-T1 through U3-T6 verbatim from the contract, unedited).

## 3. Local-mirror removal (task step 2)

- Deleted `apps/mobile/src/build/lld-v1-local.ts` (the placeholder mirror; field names had already
  been kept identical to the contract doc, confirmed byte-for-byte against the real `lld-v1.ts`
  before deleting).
- `apps/mobile/src/build/LldReadyGate.ts`: `import type { KilledAlt, ModuleBrief } from
  './lld-v1-local'` → `from './lld-v1'`. One-line change, as the original reconciliation note in the
  file predicted. Updated the file's doc comment to reflect the completed reconciliation instead of
  leaving the forward-looking note in place.

## 4. Test results (real output, not summarized)

### Targeted (U1 + U3 suites)

```
 RUN  v3.2.7 .../sot-lld-ready-gate

 ✓ apps/mobile/src/build/lld-v1.test.ts (5 tests) 146ms
 ✓ apps/mobile/src/build/LldReadyGate.test.ts (6 tests) 111ms

 Test Files  2 passed (2)
      Tests  11 passed (11)
```

Full output: `targeted-test-output.txt`.

### Whole-repo `npx vitest run`

```
 Test Files  3 failed | 55 passed (58)
      Tests  3 failed | 707 passed (710)
```

The 3 failing files are **pre-existing and unrelated to this unit**:

- `backend/gateway-sidecar/__tests__/adapterTelemetry.test.ts` and `.../complete.test.ts` — both fail
  to *load* (`Cannot find module '@pe/llm-gateway/adapters/memory'`), the same registry
  module-resolution gap multiple other 2026-09-02 fleet entries in `FLEET-LEARNINGS.md` document.
- `apps/mobile/src/presence/NoiseEngine.test.ts` — 3 sub-tests time out at the hard-coded 5000ms
  budget under concurrent multi-agent machine load (this session had ~15+ parallel `rustc`/`cargo`
  processes running from sibling worktrees at the time — confirmed via `ps aux`). Documented as
  flaky-under-load in `FLEET-LEARNINGS.md`'s `listening-state-delivery-receipt` entry.

**Isolation, not assumption:** created a disposable detached worktree
(`git worktree add --detach /tmp/reconcile-evidence/baseline-main main`, per this task's explicit
instruction and the FLEET-LEARNINGS `git stash`-hazard warning) with symlinked `node_modules` /
`.venv`, and re-ran exactly these 3 failing files against unmodified `main` — **identical failure
set** (2 files fail to load on the same missing module; the 2 NoiseEngine perf sub-tests time out the
same way). Confirms these are pre-existing, not caused by this reconciliation. Worktree removed
after the check (`git worktree remove --force`).

Full output: `full-vitest-output.txt`.

### `npm run verify`

Exits 2, halting at `typecheck`:

```
apps/mobile/src/AppAudioBedFallback.test.tsx(171,9): error TS2345: Argument of type 'AtomizerPort' is
not assignable to parameter of type 'number'.
```

This file is **not owned by U3** (contract §6 "Owns" column for `lld-ready-gate` is
`apps/mobile/src/build/LldReadyGate.ts` only) and the error is unrelated to `ModuleBrief`/`lld-v1`.
Confirmed pre-existing by isolating the same disposable-worktree method against unmodified `main`
(now at `19a32af`, since another concurrent session advanced it further mid-task): byte-identical
error, same file, same line. `apps/mobile/src/build/` itself typechecks clean —
`npx tsc -p apps/mobile --noEmit 2>&1 | grep 'build/'` returns nothing.

Because `verify`'s `typecheck` step halts before reaching `test`/`test:py`/`test:rs`, those three
were run individually instead, to get real evidence for every layer this unit touches:

- `npx vitest run` (whole repo): see above, 707/710 passing, 3 pre-existing-and-isolated failures.
- `npm run test:py`: **395 passed**, 0 failed (full output: `test-py-output.txt`).
- `npm run test:rs`: **64 passed**, 0 failed (full output: `test-rs-output.txt`).
- `npm run lint` (`node tooling/boundary-lint.mjs`): clean.

Full `verify` transcript (showing the real halt point, not a summary): `verify-output.txt`.

## 5. Files touched by this reconciliation (on top of the rebased `ff42d43`→`72a077b`)

- `apps/mobile/src/build/LldReadyGate.ts` — import repoint + doc comment update (2 lines net).
- `apps/mobile/src/build/lld-v1-local.ts` — deleted.
- `apps/mobile/src/build/fixtures/complete_module.json` — replaced with U1's canonical fixture
  (conflict resolution during rebase).
- `apps/mobile/src/build/fixtures/one_line_freeze.json` — replaced with U1's canonical fixture
  (conflict resolution during rebase).
- `apps/mobile/src/build/fixtures/AUTHORSHIP.md` — reconciled, records the re-verification.
- `.gitignore` — conflict resolved to `main`'s wording (functionally identical to U3's own fix).

No file outside `apps/mobile/src/build/` and `.gitignore` was touched. No acceptance test file
(`apps/mobile/src/build/LldReadyGate.test.ts`) was edited — byte-identical to the version the
contract froze.

## 6. Reproduce from a fresh clone

```bash
git clone <repo> && cd <repo>
git checkout sot-p0/lld-ready-gate
npm ci   # or symlink node_modules from a sibling checkout if this is a nested worktree
cd backend/relay-py && python3 -m venv .venv && .venv/bin/pip install -e ".[dev]" && cd ../..
npx vitest run apps/mobile/src/build/LldReadyGate.test.ts apps/mobile/src/build/lld-v1.test.ts
npm run test:py
npm run test:rs
npm run lint
npx tsc -p apps/mobile --noEmit   # apps/mobile/src/build/ specifically is clean; one pre-existing
                                  # unrelated error remains in AppAudioBedFallback.test.tsx
```

## 7. Verdict: is U3 ready to merge?

**Yes.** All of U3's own acceptance criteria (contract §6: "all 14 checks fire independently; both
fixtures give the right verdict; purity proven") are met against the **real, merged** `lld.v1`
schema and the **real, independently-authored** fixtures — not the placeholder mirror or
self-authored fixtures the original build used. The unit touches only its owned file
(`apps/mobile/src/build/LldReadyGate.ts`) plus the shared fixtures/`.gitignore` reconciliation every
U1-dependent unit needs. The acceptance test file is byte-unchanged from the contract. The only red
in `npm run verify` is a pre-existing, independently-confirmed, out-of-scope gap
(`AppAudioBedFallback.test.tsx`) that predates this branch and is present on unmodified `main`.
