# Adversarial review: opus walkthrough

Date: 2026-08-25  
Deliverable: `docs/delta.d/opus-walkthrough.md`  
Contract used: `handover/BACKLOG.md:92-106`, B8 (`Every non-zero exit must print a reason`).

No standalone `opus-walkthrough` heading exists in `handover/BACKLOG.md`; the only reference is
B8. That contract is therefore the nearest explicit acceptance contract, not an inferred green
status.

## Verdict

**REJECT**

The walkthrough records real defects, but it is not acceptance evidence. One cited command does
not reproduce its own claim, the successful end-to-end flow is not reproducible from the document,
the required M9 detector is absent, mutation evidence is absent, and the required verifier is red.

## What was claimed

The deliverable claims:

1. A `plan` -> refusal -> SOW -> accept -> `run` -> status -> ledger verify -> tamper -> restore
   flow worked end to end.
2. `fleet run --task ""` exits 7 with zero bytes on stdout and stderr.
3. Tampered `fleet ledger verify` exits 8 with no output.
4. Unset `FLEET_STATE` makes `fleet plan` print `intent:`, `agent:`, and `skills:` before the
   environment fault.
5. A detector should require a reason line for every non-zero exit.

The prose says “Two defects” and then adds “A third”. The failure cases discussed are three, not
two. No walkthrough `checked/total` denominator is published, and no exact state, repository,
task, SOW id, artifact id, ledger count, tamper operation, or command transcript is included for
the claimed successful flow.

## Commands actually run

All commands ran from the quoted repository path. Fleet commands used the checkout's built binary
`keel/target/release/fleet` via `PATH` and disposable state directories. No `git` command was run.

| Command/case | Exit | stdout bytes | stderr bytes | Observation |
|---|---:|---:|---:|---|
| `fleet run --task ""` with `FLEET_STATE` unset | 3 | 0 | 64 | Environment refusal: `MISSING_FLEET_STATE`; it does not exercise empty-task handling. |
| `fleet run --task "" --repo <clean-target> --agent stub` | 7 | 0 | 0 | Valid-argument empty task silently refuses; the underlying defect is real. A receipt records `EMPTY_TASK`. |
| `env -u FLEET_STATE fleet plan` | 7 | 0 | 41 | Usage refusal only; no partial plan. |
| `env -u FLEET_STATE fleet plan "add a --version flag to the cli"` | 3 | 55 | 264 | Prints `intent`, `agent`, and `skills` before the environment reason. |
| `fleet plan "add a --version flag to the cli"` | 0 | 499 | 0 | Plan renders and reports an unavailable routed lane; it executes nothing. |
| `fleet plan "make me a sandwich"` | 7 | 0 | 154 | Refusal names three closest intents. |
| incomplete `fleet sow --task ...` | 7 | 0 | 806 | Refusal names the missing challenge citation and prints a corrected template. |
| complete `fleet sow --task ...` | 9 | 1524 | 194 | SOW was emitted and an id was returned. |
| `fleet sow accept --id <returned-id>` | 0 | 125 | 0 | Acceptance receipt was written. |
| accepted `fleet run ... --agent stub` on a minimal clean disposable target | 6 | 0 | 204 | Stub adapter exited without an fd-3 result; no successful artifact or attestation was observed. |
| tampered `fleet ledger verify` | 8 | 0 | 0 | Silent mismatch reproduced. |
| restored `fleet ledger verify` | 0 | 27 | 0 | `verified checked=2 total=2`. |
| bogus `fleet attest verify 0094d2237b98a12d` | 8 | 0 | 0 | Another silent verification mismatch omitted by the walkthrough. |
| fresh `fleet status --json` | 0 | 452 | 0 | Returns `checked=0`, `total=0`, `empty=true`: vacuous success. |
| fresh `fleet ledger verify` | 6 | 0 | 0 | Empty-ledger invariant failure is also silent. |
| `bash tests/corpus/M9.sh` | 127 | 0 | 62 | `tests/corpus/M9.sh: No such file or directory`. |

The accepted-run attempt against the first copied target was refused with exit 7 because copied
graph/build files made the target dirty. I then constructed a minimal copy containing only its
tracked-looking files; that reached the real stub adapter and failed with exit 6. This is an
environment/adapter limitation, not evidence that the documented end-to-end flow works.

Implementation cross-checks agree with the runtime:

- `keel/fleet/src/main.rs:904-912` records `EMPTY_TASK` and returns without printing.
- `keel/fleet/src/main.rs:2995-3000` prints ledger success only after `verify_rows` succeeds;
  mismatch errors return before any diagnostic.
- `keel/fleet/src/main.rs:2646-2712` prints the plan fields before calling `state_dir()`.

## Independent verifier

Exact command run:

```text
FLEET_MUTANTS=0 bash verify.sh
```

Exit: **6**.

Final wrapper output:

```text
FAIL corpus                     (see var/verify.log)
-- 14 passed, 1 failed, 1 skipped (denominator: 16 stages) --
```

The skipped mutants stage is explicitly not mutation evidence. `var/verify.log` ended with:

```text
DENOMINATOR checked=34 total=34 excluded=69 caught=25
```

There are 105 `tests/corpus/*.sh` detectors and 105 manifest lines. The published verifier
arithmetic accounts for 34 checked plus 69 excluded = 103, leaving two detector inputs
unexplained. The log also records multiple 30-second timeout failures, including A1, A3, A4,
A8, B10, C1, C2, C3, C6, C8, C9, C11, C22, C24, C26, S4, S6, S9, T1, T15, T20, and T5/T6.

## Required changes before acceptance

1. Restore an explicit `opus-walkthrough` backlog item, or explicitly identify B8 as the contract
   for this deliverable. Do not leave the acceptance target implicit.
2. Correct the walkthrough transcript. Include the required `--repo` and `--agent` arguments for
   the empty-task case, exact disposable state/repository setup, exact task and SOW data, every
   exit code, stream result, artifact/status/ledger count, tamper operation, and restore command.
   Publish a walkthrough `checked/total` denominator and classify every omitted or untriggerable
   surface.
3. Fix the user-facing paths: emit a reason for valid-argument empty-task refusal, tampered-ledger
   mismatch, attest mismatch, empty-ledger failure, and any other non-zero verification path.
   Check the environment before printing plan fields so an environment refusal does not leak a
   partial plan.
4. Add `tests/corpus/M9.sh`. It must exercise every refusable surface, publish `checked/total`,
   fail when it checks zero inputs, and avoid matching its own documentation. Assert at least one
   reason line for every non-zero exit.
5. Mutation-test M9 in both directions: removing the reason must make it red, and restoring the
   reason must make it green. Record the actual mutation result; `FLEET_MUTANTS=0` is a skip.
6. Reconcile the corpus denominator so checked, excluded, and total account for all 105 inputs,
   then rerun `FLEET_MUTANTS=0 bash verify.sh` to exit 0 with no failed stage. Do not claim B8
   accepted while the wrapper is `14 passed, 1 failed, 1 skipped`.
