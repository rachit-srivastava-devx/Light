# Adversarial review — opus walkthrough

Date: 2026-08-25
Reviewer: adversarial independent run
Deliverable: `docs/delta.d/opus-walkthrough.md`
Contract used: B8 in `handover/BACKLOG.md` (the backlog has no literal `opus-walkthrough` item; B8 is the item that names this deliverable).

## Verdict

**REJECT**

The walkthrough identifies two real silent failure paths, but it is not a reproducible user test as
written, its proposed detector would not catch all three reported defects, the required M9 detector
is absent, and the required verifier is red.

## What was claimed

- A complete `plan` → SOW refusal → accepted SOW → `run` → `status` → ledger verify → tamper →
  restore flow works end to end.
- `fleet run --task ""` exits 7 with zero bytes on both streams.
- A tampered `fleet ledger verify` exits 8 silently; a clean chain prints
  `verified checked=12 total=12`.
- An unset-state plan prints `intent:`, `agent:`, and `skills:` before the environment refusal.
- A detector requiring one reason line for every non-zero exit would catch all three defects.

## Commands actually run

All commands used the explicit `keel/target/release/fleet` binary and disposable `FLEET_STATE`
directories. No shell `git` command was run by this review.

| Command / case | Exit | Observed result |
|---|---:|---|
| Exact `fleet run --task ""` form | 7 | stdout 0 bytes; stderr 187 bytes: `--repo is required` and usage. The exact shorthand in the deliverable does not reproduce the claimed silence. |
| `fleet run --task "" --repo "$PWD" --agent stub` | 7 | stdout 0; stderr 0. This reproduces the silent `EMPTY_TASK` refusal. |
| `env -u FLEET_STATE fleet plan "add a --version flag to the cli"` | 3 | stdout 55 bytes containing the three plan lines; stderr 264 bytes naming missing `FLEET_STATE`. Partial output reproduced. |
| Exact bare `env -u FLEET_STATE fleet plan` | 7 | stdout 0; stderr 41 bytes with usage. The bare shorthand does not reproduce the partial-output claim. |
| `fleet plan "add a --version flag to the cli"` with isolated state | 0 | 3 planned commands, denominator 3; route unavailable with a reason. |
| `fleet plan "make me a sandwich"` | 7 | Refusal with 3 closest candidates. |
| `fleet run ... --agent stub` before SOW acceptance | 7 | Refusal names the SOW id and acceptance commands; stdout 0, stderr 290. |
| `fleet sow --task "add a --version flag"` | 7 | SOW refusal names the missing citation and prints a corrected template; stdout 0, stderr 806. |
| Complete SOW command | 9 | SOW-ready JSON on stdout and acceptance instruction on stderr. |
| `fleet sow accept --id <printed-id>` | 0 | `SOW_ACCEPTED` receipt emitted. |
| Accepted `fleet run ... --repo "$PWD" --agent stub` | 7 | The shared checkout was dirty, so the user run refused with `TARGET_REPO_NOT_CLEAN`; stderr 519. The walkthrough omits the required clean-target precondition and exact task/repo/agent setup. |
| `fleet status` after the refused flow | 0 | `2 of 2` receipts; DONE 0 + PENDING 0 + FAILED 2 + NEEDS-ITERATION 0 = 2. |
| Clean one-row ledger verify | 0 | `verified checked=1 total=1`. |
| Empty-state `fleet ledger verify` | 6 | stdout 0; stderr 0. The zero-input invariant fails, rather than passing vacuously. |
| 12 valid ledger appends | 0 each | 12 of 12 append commands succeeded. |
| 12-row clean `fleet ledger verify` | 0 | `verified checked=12 total=12`. |
| 12-row tampered `fleet ledger verify` | 8 | stdout 0; stderr 0. Silent mismatch reproduced exactly. |
| Restored 12-row `fleet ledger verify` | 0 | `verified checked=12 total=12`. |

## Required independent verifier

Command:

```text
FLEET_MUTANTS=0 bash verify.sh
```

Real terminal result:

```text
== fleet verify ==
  ok   fmt
  ok   clippy -D warn
  ok   unit tests
  ok   acceptance builds
  ok   cargo-deny
  ok   cargo-audit
  ok   secrets
  ok   acceptance
  ok   readme
  ok   swarm
  ok   policy
  SKIPPED WITH A REASON mutants (set FLEET_MUTANTS=1 - full pass ~24min)
  ok   attest-smoke
  ok   pytest
  ok   detectors
  FAIL corpus (see var/verify.log)
-- 14 passed, 1 failed, 1 skipped (denominator: 16 stages) --
```

The command exited **6**. `var/verify.log` reports:

```text
  TIMEOUT A4.sh exceeded 30s -- treated as FAILED
  TIMEOUT C11.sh exceeded 30s -- treated as FAILED
  TIMEOUT C2.sh exceeded 30s -- treated as FAILED
  TIMEOUT C22.sh exceeded 30s -- treated as FAILED
  TIMEOUT C6.sh exceeded 30s -- treated as FAILED
  TIMEOUT S6.sh exceeded 30s -- treated as FAILED
  TIMEOUT S9.sh exceeded 30s -- treated as FAILED
  TIMEOUT T1.sh exceeded 30s -- treated as FAILED
  TIMEOUT T15.sh exceeded 30s -- treated as FAILED
DENOMINATOR checked=34 total=34 excluded=69 caught=12
```

Arithmetic check: 105 corpus shell files − `run.sh` − `_selftest.sh` = 103 candidates; 103 =
34 checked + 69 excluded. The corpus did examine 34 inputs, but nine timed out and were counted as
failures; the stage therefore is not green. Mutants were not tested in this required run.

## Findings

### F1 — The cited empty-task command is incomplete

The exact command in the deliverable fails argument validation before empty-task validation and
prints a useful error. Only the fully specified form reproduces the silent path. A user cannot
reproduce the central claim from the documented command.

### F2 — The proposed detector does not catch the third defect

The unset-state plan writes a reason to stderr (`FLEET_STATE is not set`), so a detector asserting
“every non-zero exit writes at least one line naming the reason” would pass it. It does not detect
that user-facing plan output was already written before the refusal. The detector must separately
assert output ordering/no partial plan, or the claim that it catches all three must be removed.

### F3 — No denominator is published for the walkthrough coverage

`checked=12 total=12` is only the ledger-row denominator. The document does not publish how many
commands/surfaces were exercised, how many passed or failed, or whether the three listed failure
cases are the complete tested set. B8 explicitly requires M9 to publish its denominator.

### F4 — M9 and mutation evidence are missing

`tests/corpus/M9.sh` does not exist. B8 remains unchecked. The required verifier also explicitly
skipped mutation testing, so the acceptance requirement “M9 exists ... and is mutation-tested” is
not evidenced.

### F5 — The end-to-end success claim is not reproducible from this document

The accepted-SOW run was blocked by the shared checkout’s dirty-tree guard. That is an expected
operator precondition, not proof that the implementation is broken, but the walkthrough omits the
clean target repository, exact task text, binary path, state setup, and `--repo`/`--agent` arguments
needed to rerun the claimed success path. The document therefore cannot be independently replayed
as a user flow.

## Exactly what must change for acceptance

1. Replace shorthand commands with complete, copy-pastable commands including binary path,
   `FLEET_STATE`, `--repo`, `--agent`, exact SOW task text, and a clean disposable target.
2. Add a denominator table covering every exercised surface and classify all three failure cases;
   retain the ledger denominator separately.
3. Add `tests/corpus/M9.sh` with a non-zero-exit denominator and two distinct assertions: a reason
   is emitted, and no partial plan is emitted before an environment refusal. Ensure it does not
   match its own documentation.
4. Mutation-test M9 in both directions, publish the caught/total result, and rerun
   `FLEET_MUTANTS=0 bash verify.sh` until the final exit is 0 with the complete 16-stage denominator.

