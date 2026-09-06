# Adversarial review: opus-walkthrough

Review ID: `OPUS-WALKTHROUGH-ADVERSARIAL-20260825-CODEX-LOCAL`

Deliverable: `docs/delta.d/opus-walkthrough.md`

Contract used: B8 in `handover/BACKLOG.md`. There is no standalone
`opus-walkthrough` heading in the current backlog; B8 is the only item that names the
deliverable. B8 is still `[ ]` and requires all three paths to be fixed, `tests/corpus/M9.sh`
with a published denominator and mutation evidence, and a green verifier.

## What was claimed

The walkthrough says a complete plan/SOW/run/status/ledger/tamper/restore flow worked, then
reports three user-facing failure paths:

1. Empty `run` returns exit 7 with no stdout or stderr.
2. Tampered `ledger verify` returns exit 8 with no output.
3. `plan` with `FLEET_STATE` unset prints a partial plan before its environment failure.

It proposes a detector requiring every non-zero exit to print a named reason. It publishes no
walkthrough denominator, no per-surface coverage count, no M9 result, and no mutation result.

## What I actually ran

All commands used the existing release binary at `keel/target/release/fleet`; `fleet` is not on
`PATH`. Temporary state directories were created outside the repository.

| Command | Exit | Observed |
|---|---:|---|
| `FLEET_STATE=<tmp> keel/target/release/fleet run --task ""` | 7 | Not the claimed silent result: it prints `--repo is required` and usage text. The documented command is incomplete. |
| `FLEET_STATE=<tmp> keel/target/release/fleet run --task "" --repo . --agent stub` | 7 | Reproduces the defect: stdout 0 bytes and stderr 0 bytes. |
| `env -u FLEET_STATE keel/target/release/fleet plan "add a --version flag"` | 3 | Prints `intent: implement a change`, `agent: builder`, and `skills: rust`, then the environment-fault diagnostic. |
| `FLEET_STATE=<tmp> keel/target/release/fleet ledger append --event note --body '{}'` | 0 | Created a one-row valid ledger. |
| `FLEET_STATE=<tmp> keel/target/release/fleet ledger verify` on that ledger | 0 | Printed `verified checked=1 total=1`. |
| Same ledger after changing its JSON body without recomputing the hash | 8 | Reproduces the defect: stdout 0 bytes and stderr 0 bytes. |
| `FLEET_STATE=<fresh-tmp> keel/target/release/fleet ledger verify` on an empty ledger | 6 | Additional silent non-zero path: stdout 0 bytes and stderr 0 bytes. |
| `bash tests/corpus/M9.sh` | 127 | `tests/corpus/M9.sh: No such file or directory`. |

The source agrees with the runtime: `ledger_verify` calls `verify_rows` and prints only after
success (`keel/fleet/src/main.rs:2995-3000`), while `plan_command` prints the three plan fields
before calling `state_dir()` (`keel/fleet/src/main.rs:2659-2664`). The existing acceptance test
checks only the empty-run exit code and receipt count (`tests/acceptance/p0.sh:108-113`), so it
does not detect the empty output.

## Independent verifier

Command run exactly:

```text
FLEET_MUTANTS=0 bash verify.sh
```

Final output:

```text
  FAIL corpus                     (see var/verify.log)
-- 14 passed, 1 failed, 1 skipped (denominator: 16 stages) --

VERIFY_EXIT_CODE=6
```

The failing corpus log reported:

```text
M6: 22 of 23 documented commands exist (denominator: 23)
DENOMINATOR checked=34 total=34 excluded=69 caught=21
```

The corpus also reported multiple detectors timing out at 30 seconds, each treated as failed,
including A8, B10, C2, C6, C8, C11, C22, C24, C26, S6, S9, T1, T5, T6, and T20. The mutants
stage was explicitly skipped because `FLEET_MUTANTS=0`; that is not mutation evidence for M9.

Arithmetic: the corpus runner's `34 checked + 69 excluded = 103` detector files, while the
directory contains 105 shell files because `run.sh` and `_selftest.sh` are excluded. That
denominator is for the existing corpus, not proof of M9 coverage. The walkthrough itself has no
published denominator for its claimed end-to-end steps.

## Findings

### F1 — REJECT: the two primary silent failures remain

The full-argument empty-run and tampered-ledger commands still return non-zero with zero bytes
on both streams. These are the exact user-facing defects the walkthrough identifies, not merely
documentation problems.

### F2 — REJECT: the required M9 detector is absent

`tests/corpus/M9.sh` does not exist and its required command exits 127. Therefore there is no
detector denominator, no exhaustive refusable-surface coverage, and no mutation evidence. A
generic existing corpus count cannot substitute for the specifically required M9 contract.

### F3 — REJECT: the verifier is red, not green

`FLEET_MUTANTS=0 bash verify.sh` exited 6 with one failed stage. The corpus denominator includes
21 caught failures and timed-out detectors, and M6 reports 22/23 documented commands. The
intermediate green stages do not establish a green verifier.

### F4 — Finding: the walkthrough is not reproducible as written

The cited empty-run command omits required `--repo` and `--agent` arguments, so running it
literally produces a usage diagnostic rather than the claimed silent failure. The successful
12-row ledger output is also not generated by any command in the document; the only reproducible
clean ledger I created verified as `checked=1 total=1`. The walkthrough needs exact setup,
commands, teardown, and a step denominator.

### F5 — Finding: the contract name is ambiguous

`handover/BACKLOG.md` has no literal `opus-walkthrough` acceptance item. It only references this
file from B8. Either add the named item or explicitly state that B8 is the governing contract;
otherwise a reviewer cannot map the requested item to acceptance criteria without inference.

## Verdict: REJECT

The walkthrough correctly identifies real runtime defects, but it is not acceptance-complete and
the surrounding B8 work is not done.

## Required changes for acceptance

1. Fix empty `run` and tampered/empty `ledger verify` so every non-zero exit emits a human-readable
   reason, while preserving the typed exit codes and refusal receipts.
2. Fix `plan` to validate required environment before emitting plan content, or otherwise make
   its failure output a complete, intentional diagnostic rather than a partial plan.
3. Add `tests/corpus/M9.sh` that enumerates every reachable refusable surface, executes each one,
   asserts a non-zero exit has a reason line, and fails on an empty/untriggerable measurement.
4. Publish M9's `checked`, `total`, exclusions, and caught counts. Mutation-test the reason
   assertions in both directions: remove/bypass the assertion and prove M9 goes red, then restore
   it and prove green.
5. Fix the existing red corpus/M6 issues and rerun `FLEET_MUTANTS=0 bash verify.sh` to a final
   `0` exit with the complete `passed, failed, skipped (denominator: ...)` line.
6. Add exact reproducible walkthrough commands and a denominator, and resolve the missing
   `opus-walkthrough` versus B8 contract naming.
