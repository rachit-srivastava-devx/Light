# Adversarial review — opus walkthrough

Date: 2026-08-25  
Reviewer: adversarial reviewer  
Contract: `handover/BACKLOG.md` B8, lines 92–106

## Verdict: REJECT

The walkthrough found real defects, but it is not an acceptance-complete B8
deliverable. One command is under-specified and therefore reports a stale
literal result; two user-visible failure paths remain silent; `M9.sh` does not
exist; no M9 denominator or mutation evidence is published; and the required
gate is red.

## What was claimed

`docs/delta.d/opus-walkthrough.md` claims that:

1. `fleet run --task ""` exits 7 with zero bytes on stdout and stderr.
2. `fleet ledger verify` exits 8 with no output after ledger tampering, while
   success prints `verified checked=12 total=12`.
3. `fleet plan` with `FLEET_STATE` unset prints `intent:`, `agent:`, and
   `skills:` before the environment refusal.
4. These are mechanisable by a detector requiring every non-zero exit to name
   its reason.

B8 additionally requires all three paths to be fixed, `tests/corpus/M9.sh` to
   exist with a published denominator and mutation test, and `verify.sh` to be
   green.

## Commands actually run

All runs used the existing `keel/target/debug/fleet` binary and isolated
temporary state directories.

| Command / scenario | Exit | Observed output |
|---|---:|---|
| `FLEET_STATE="$state" keel/target/debug/fleet run --task ""` | 7 | stdout 0 bytes; stderr 187 bytes: `fleet: run: --repo is required` |
| `FLEET_STATE="$state" keel/target/debug/fleet run --task "" --repo . --agent stub` | 7 | stdout 0 bytes; stderr 0 bytes — the underlying silent empty-task refusal is confirmed once required CLI arguments are supplied |
| `env -u FLEET_STATE keel/target/debug/fleet plan "add a --version flag to the cli"` | 3 | stdout 55 bytes containing exactly `intent:`, `agent:`, `skills:`; stderr 264 bytes with the environment fault |
| append two ledger rows, verify intact | 0 | `verified checked=2 total=2` |
| tamper a ledger body, then `FLEET_STATE="$state" keel/target/debug/fleet ledger verify` | 8 | stdout 0 bytes; stderr 0 bytes — silent mismatch confirmed |
| restore the ledger, verify again | 0 | `verified checked=2 total=2` |
| `FLEET_MUTANTS=0 bash verify.sh` | 6 | `FAIL corpus`; `-- 14 passed, 1 failed, 1 skipped (denominator: 16 stages) --` |

The literal `fleet run --task ""` claim is therefore incomplete: current CLI
parsing stops first on the missing required `--repo`. The intended empty-task
case is still a genuine silent failure with the required arguments present.

The arithmetic observed is internally consistent: the ledger check examined
2 of 2 rows, and the gate totals add to 14 + 1 + 1 = 16. The claimed 12 of 12
success case is not reproducible from the walkthrough because it supplies no
fixture or command sequence for constructing those 12 rows; the 2 of 2 run
confirms the denominator behavior without validating that historical count.

## Acceptance gaps

- `tests/corpus/M9.sh` is absent (`M9_PRESENT=no`). There is no published M9
  denominator and no evidence that every refusable surface was exercised.
- No mutation test of M9 exists. The gate's mutant stage was intentionally
  skipped under `FLEET_MUTANTS=0`; that does not satisfy B8's separate
  mutation-tested requirement.
- The required verifier is red: corpus failed with exit 6. A green set of
  earlier stages does not satisfy the contract.
- The empty-task command in the walkthrough omits mandatory `--repo` and
  `--agent` arguments, so its literal reproduction does not reach the claimed
  code path.

## Required changes for acceptance

1. Fix the empty-task refusal, ledger mismatch refusal, and plan environment
   preflight so every non-zero exit emits at least one human-readable line
   naming the reason; preflight `FLEET_STATE` before emitting plan lines.
2. Correct the walkthrough to show complete, runnable commands and publish the
   fixture inputs and denominator for the ledger run.
3. Add `tests/corpus/M9.sh`. It must drive every refusable surface, fail on an
   empty input set, assert a reason line for each non-zero exit, and publish
   `checked/total` (including hard cases, not silently excluding them).
4. Mutation-test M9 by removing or bypassing its reason-output assertion,
   capture the required red result, restore the guard, and capture green.
5. Rerun `FLEET_MUTANTS=0 bash verify.sh` to a terminal green result with
   `0 failed`, and publish its final denominator and exit code.

