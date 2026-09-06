# U4 build-mode-wire — evidence report

Task: Speed-of-Thought P0 contract (`docs/SPEED-OF-THOUGHT-P0-CONTRACT.md`) unit U4
(`build-mode-wire`). Worktree: `adhd-focus-orb-worktrees/sot-build-mode-wire`, branch
`sot-p0/build-mode-wire`.

## Registry decision (C1/L2)

Stated by the contract itself: **extract + build-new**. For U4 specifically:
- `OrbMode` (router/contracts.ts) and `ResponseMode` (proxy/schemas.py) — **extract**: adding one
  literal/enum member to an existing, already-wired type, per the contract's diff-shape (§1.3).
- `FreezeStore` (store/freeze_store.py) — **build-new**, modelled on the existing
  `ConversationStore` shape but genuinely new code (append-only, no prune, no update/delete).
- `lld_decomposer.py` — **build-new**, a sibling to `proxy/atomizer.py`, reusing its
  `_strip_markdown_fence`/repair-once/fail-closed shape but a new module (contract §1.2 explicitly
  rejects genericizing `atomize()` itself).

## Starting state

- Worktree started at base commit `e942446` (0 commits of its own). U1 (`lld-v1-schema`) was
  already merged to `main` at `79652d9`; since my branch had zero unique commits I fast-forwarded
  (`git reset --hard main`) rather than rebase — a no-op for history, nothing of mine to replay.
  Confirmed via `git merge-base --is-ancestor HEAD main` before doing it.
- U2 (`build-fsm`) was NOT yet merged to `main` (exists only as branch `sot-p0/build-fsm`, tip
  `8814d2e`). This does not block U4: U4's owned files have no dependency on
  `BuildStateMachine.ts`/`fsm-core.ts`/`build/contracts.ts` (U2's files) — file-disjoint per the
  contract's own dependency map (§6 "U2 is fully file-disjoint from U3/U4/U5"). No reconciliation
  action needed on U4's side once U2 merges.
- `apps/mobile/src/build/lld-v1.ts` (U1) already existed on `main`. U4's own files don't import it:
  `BuildModeWire.test.ts` only imports `createRelayConversationPort`, and the Python side reuses the
  already-existing `lld_schemas.py`'s `validate_module_brief`/`ModuleBrief` (also from U1).

## Files changed (all within U4's owns column, plus the two named acceptance test files)

- `apps/mobile/src/router/contracts.ts` — added `'build'` to the `OrbMode` union (§1.3).
- `backend/relay-py/src/orb_relay/proxy/schemas.py` — added `BUILD = "build"` to `ResponseMode` (§1.3).
- `backend/relay-py/src/orb_relay/store/freeze_store.py` — **new**, `FreezeStore` (§1.5).
- `backend/relay-py/src/orb_relay/proxy/lld_decomposer.py` — **new**, `decompose()` (§1.2).
- `apps/mobile/src/build/BuildModeWire.test.ts` — **new**, byte-identical to the contract's U4 test.
- `backend/relay-py/tests/test_freeze_store_append_only.py` — **new**, byte-identical to the contract.

No file outside this list was touched. Per the contract's explicit "UNCHANGED"/forbidden markers:
`apps/mobile/src/runtime/ConversationPort.ts`, `backend/relay-py/src/orb_relay/proxy/atomizer.py`,
`apps/mobile/src/session/*`, `apps/mobile/src/lld/ResponseEnvelope.ts`,
`apps/mobile/src/lld/ClarifyProtocol.ts` were all left untouched (`git status --short` confirms
nothing else modified).

## RED confirmed (both acceptance test files, greenfield)

Before writing `freeze_store.py`/`lld_decomposer.py`:
```
$ backend/relay-py/.venv/bin/python -m pytest -q backend/relay-py/tests/test_freeze_store_append_only.py
ModuleNotFoundError: No module named 'orb_relay.store.freeze_store'
1 error during collection
```
Before adding `'build'` to `OrbMode`, `tsc -p apps/mobile` on `BuildModeWire.test.ts` line 12:
```
error TS2322: Type '"build"' is not assignable to type 'OrbMode | undefined'.
```
(`vitest run` alone did not show RED for U4-T1 because JS execution doesn't typecheck — the runtime
already forwards any string mode verbatim; the real RED for this file is at the `tsc` layer.)

## GREEN

```
$ npx vitest run apps/mobile/src/build/BuildModeWire.test.ts
 ✓ apps/mobile/src/build/BuildModeWire.test.ts > build mode reaches the wire > U4-T1 mode:'build' appears in the POST body, not just in local state
 ✓ apps/mobile/src/build/BuildModeWire.test.ts > build mode reaches the wire > U4-T2 an absent mode is omitted, not sent as null (relay would 422)
 Test Files  1 passed (1)
      Tests  2 passed (2)
```
Real request bodies (see `real-request-bodies.log`, via a throwaway scratch test created and
deleted, not part of the acceptance suite):
```
U4-T1 body: {"session_id":"s","tenant_id":"t","user_id":"u","text":"build me a thing","mode":"build"}
U4-T2 body: {"session_id":"s","tenant_id":"t","user_id":"u","text":"hi"}
```

```
$ PYTHONPATH=.../backend/relay-py/src PYTHONDONTWRITEBYTECODE=1 .venv/bin/pytest -q backend/relay-py/tests/test_freeze_store_append_only.py
...                                                                      [100%]
3 passed in 5.26s
```
Real ledger + decompose evidence (see `freeze-store-and-decomposer-evidence.log`):
```
list_freezes: [{'freeze_id': 'fz-...0001', 'version': 1, 'supersedes': None, ...},
               {'freeze_id': 'fz-...0002', 'version': 2, 'supersedes': 'fz-...0001', ...}]
no update/delete/edit/set_superseded/prune: True
decompose() fail-closed result: {'kind': 'clarify_request', 'brief': None,
                                  'missing': ('interface', 'data_owned', 'acceptance', 'registry_verdict')}
```

## Whole-repo suites (post-fix)

- `npx vitest run` (whole repo): **704 passed**, 2 failed (both `NoiseEngine.test.ts` perf-timeout
  flakes under concurrent multi-agent CPU load on this shared machine — pre-existing, unrelated;
  prior 2026-09-02 FLEET-LEARNINGS entries already document this exact flake class), 3 suites fail
  to *load* on the pre-existing `@pe/llm-gateway` module-resolution gap (unrelated to U4, documented
  by prior entries this session). My own diff adds exactly the 2 `BuildModeWire.test.ts` tests.
- `backend/relay-py` full pytest: **398 passed, 0 failed** (includes my 3 new
  `test_freeze_store_append_only.py` tests; no regressions).
- `node tooling/boundary-lint.mjs`: **clean**.
- `cargo test` (backend/relay-rs): untouched by this unit (no Rust files in scope). Kicked off in
  the background; ran slowly (25+ minutes) under heavy concurrent cargo-build load from sibling
  fleet worktrees on this shared machine (confirmed via `ps aux`: several other worktrees' `cargo
  test`/`cargo build` processes running concurrently, all doing first-time uncached compiles) but
  did complete: **64 passed, 0 failed, exit code 0** (`cargo-test.log`). Consistent with this unit
  touching zero `.rs` files — no regression expected or found.

## `tsc -p apps/mobile` — isolated against a real pre-existing baseline

I isolated my diff's effect on `npm run verify`'s `typecheck` step using a **disposable detached
worktree** (`git worktree add --detach /tmp/baseline-check-u4 main`), per FLEET-LEARNINGS' explicit
instruction never to use `git stash` on this shared multi-worktree repo:

- **Baseline (`main`, unmodified, no `BuildModeWire.test.ts`)**: exactly ONE pre-existing error,
  unrelated to U4 (`tsc-baseline-main-unmodified.log`):
  ```
  apps/mobile/src/AppAudioBedFallback.test.tsx(171,9): error TS2345: Argument of type
  'AtomizerPort' is not assignable to parameter of type 'number'.
  ```
- **My worktree (after the U4 diff)**: the same one pre-existing error, plus **6 new errors, all
  confined to `apps/mobile/src/build/BuildModeWire.test.ts`** (`tsc-output.log`):
  1. Line 12 (U4-T1) and line 21 (U4-T2): `TS2345: Property 'task' is missing in type '{...}' but
     required in type 'ConversationPortInput'`.
  2. Lines 13/23 (x2 each): `TS2532`/`TS2493` — a `vi.fn(async () => ...)` with no declared
     parameters types its call-tuple as `[]`, so `fetchImpl.mock.calls[0][1]` fails `tsc` even
     though the identical code passes `vitest run` (JS execution doesn't typecheck).

**Both are real, but neither is fixable within U4's owned files, and neither is a defect introduced
by U4's implementation logic:**

- The `task` error exists because the frozen acceptance test constructs its request object without
  a `task` field, while `ConversationPortInput extends T0FocusSessionInput` and
  `T0FocusSessionInput.task` is a required `string`. Fixing this needs editing
  `apps/mobile/src/runtime/ConversationPort.ts` (the contract's own §1.3 diff-shape explicitly marks
  it **UNCHANGED**) or `apps/mobile/src/runtime/T0FocusSession.ts` (not in U4's owns column, and not
  touched by any other Speed-of-Thought unit). Note `ConversationPort.ts`'s `respond()` never
  actually reads `input.task` when building the POST body — only `active_task`/`current_step`/
  `session_state` — so this is a narrow, pre-existing, inherited-but-unused required field,
  independent of build-mode-wire. **Verified not caused by my diff**: I ran `tsc` against the frozen
  test file alone, before touching any source file, and the identical `task`-missing error at line
  21 (U4-T2) was already present (see "RED confirmed" above — that was the very first `tsc` run,
  before any source edits).
- The `TS2532`/`TS2493` pair is the same class of issue a prior worker on this repo already hit and
  documented verbatim in `apps/mobile/src/runtime/ConversationPort.test.ts`'s own comment (lines
  40-43): "a bare `vi.fn(async () => ...)` declares no parameters, so its call tuple types as `[]`
  and `calls[0][1]` fails `tsc --noEmit` even though `vitest run` passes." That worker fixed it in
  their own (non-frozen) test with a typed `captureBody()` helper — not available here because
  `BuildModeWire.test.ts` is frozen text from the contract (may not be edited, review gate §8.1).

**Flagging forward rather than routing around it**: fixing either would require editing a file
outside U4's owned scope or editing the frozen acceptance test. Recorded here and in
FLEET-LEARNINGS for the lead's review rather than silently worked around.

`npm run verify`'s exit code is therefore still non-zero in this worktree — it already was, at
`main`, for the pre-existing `AppAudioBedFallback.test.tsx`/`@pe/llm-gateway` reasons every prior
2026-09-02 FLEET-LEARNINGS entry this session has documented. Running each gate step individually
(as above) follows the same precedent every prior entry this session used, rather than reporting a
single opaque exit code.

## Reproduce from a fresh clone

```bash
cd company/products/adhd-focus-orb-worktrees/sot-build-mode-wire   # or a fresh worktree of the branch
ln -s ../../adhd-focus-orb/node_modules node_modules                                   # or: npm ci
ln -s ../../../../adhd-focus-orb/backend/relay-py/.venv backend/relay-py/.venv         # or: scripts/setup.sh
npx vitest run apps/mobile/src/build/BuildModeWire.test.ts
cd backend/relay-py && PYTHONPATH="$(pwd)/src" PYTHONDONTWRITEBYTECODE=1 \
  .venv/bin/pytest -q -o cache_dir=/tmp/focus-orb-pytest-cache tests/test_freeze_store_append_only.py
```
(The `PYTHONPATH` override is required only when `.venv` is a symlink to a sibling checkout's own
venv, whose editable install points at that sibling's absolute path — see the FLEET-LEARNINGS entry
for this unit. A worktree with its own real `.venv` from `scripts/setup.sh` would not need it.)
