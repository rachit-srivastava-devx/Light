# Adversarial review: opus-walkthrough

Reviewed: 2026-08-25

Deliverable: `docs/delta.d/opus-walkthrough.md`

Verdict: **REJECT**

## Contract

There is no literal `opus-walkthrough` item in `handover/BACKLOG.md`. The only matching
contract is B8, which references this file. B8 is still unchecked and requires the three
failure paths to be fixed, `tests/corpus/M9.sh` to exist with a published denominator and
mutation evidence, and `verify.sh` to be green.

## What was claimed

The walkthrough claims that the plan/SOW/run/status/ledger flow works end to end, then reports:

1. Empty `run` returns exit 7 with no output.
2. Tampered `ledger verify` returns exit 8 with no output.
3. `plan` with `FLEET_STATE` unset emits partial output before exit 3.

It proposes a detector covering every non-zero exit, but gives no tested-surface denominator,
per-surface results, or mutation result.

## Commands actually run

All commands used the existing release binary at `keel/target/release/fleet` and isolated
temporary `FLEET_STATE` directories. No Git command was run.

| Command / scenario | Exit | Observed |
|---|---:|---|
| `fleet run --task ""` exactly as written | 7 | 0 stdout bytes, **187 stderr bytes**: `--repo is required` plus usage. The exact walkthrough command does not reproduce the claimed silent result. |
| `fleet run --task "" --repo "$PWD" --agent stub` | 7 | **0 stdout / 0 stderr bytes**. The valid invocation reproduces the silent refusal. |
| `env -u FLEET_STATE fleet plan "fix the bug"` | 3 | 55 stdout bytes (`intent`, `agent`, `skills`), then 264 stderr bytes naming the missing environment. The partial-output claim reproduces. |
| Complete `fleet sow --task <valid task>` | 9 | `SOW_READY_AWAITING_REVIEW` and an acceptance ID printed. |
| `fleet sow accept --id <id>` | 0 | `SOW_ACCEPTED` printed. |
| `fleet run --task <same task> --repo "$PWD" --agent stub` after acceptance | 7 | 0 stdout / 519 stderr bytes. It refused because this shared checkout has uncommitted changes, so the claimed successful end-to-end run was not reproduced. |
| `fleet ledger append --event note --body '{}'` | 0 | Appended a valid row. |
| `fleet ledger verify` before tamper | 0 | `verified checked=2 total=2`. Denominator present on success. |
| Tamper `seq:0` to `seq:9`; `fleet ledger verify` | 8 | **0 stdout / 0 stderr bytes**. The silent mismatch reproduces. |
| Restore the original chain; `fleet ledger verify` | 0 | `verified checked=2 total=2`. |
| `bash tests/corpus/M9.sh` | 127 | `No such file or directory`. Required detector is absent. |
| Fresh-state `fleet ledger verify` | 6 | **0 stdout / 0 stderr bytes**. An additional silent non-zero surface omitted by the walkthrough. |
| Fresh-state `fleet status --json` | 0 | `{ "checked": 0, "total": 0, "empty": true, ... }`. This is a vacuous success on zero inputs. |

The successful SOW and acceptance steps therefore worked, but the claimed completed run did not
resolve in this checkout. The failure was explained by the CLI, unlike the silent ledger mismatch.

## Arithmetic and acceptance checks

- The walkthrough publishes no denominator for the full flow or for the proposed “every
  non-zero exit” detector. Its `checked=2 total=2` ledger result is only the ledger’s valid-row
  count, not coverage of the flow or refusal surface.
- The repository contains 105 corpus shell detectors, but only M1 through M7 exist. M9 is absent;
  there is no M9 checked/total result and no M9 mutation result.
- The fresh `status --json` result checks zero inputs and exits 0. That violates the stated rule
  that measuring nothing must fail or be explicitly treated as an unmeasured state.
- `FLEET_MUTANTS=0 bash verify.sh` completed with exit **6** and this real final result:

  ```text
  FAIL corpus                     (see var/verify.log)
  -- 14 passed, 1 failed, 1 skipped (denominator: 16 stages) --
  ```

  The mutation stage was skipped with its documented reason. `var/verify.log` reports
  `DENOMINATOR checked=34 total=34 excluded=69 caught=12` and nine timed-out detectors treated as
  failed: A4, C11, C2, C22, C6, S6, S9, T1, and T15. The arithmetic is `14 + 1 + 1 = 16`; this is
  not a green gate.

## Findings

### F1 — REJECT: the acceptance contract named by the request is missing

`handover/BACKLOG.md` contains no `opus-walkthrough` item. B8 is the only binding item and remains
unchecked. A reviewer cannot evaluate the requested artifact against the named contract without
silently substituting B8.

### F2 — REJECT: the “full flow works end to end” claim is not reproducible

The document gives no exact task text, binary path/build identity, repository fixture, state setup,
exit codes, or output transcript. Replaying a valid SOW in the shared checkout reached the stated
acceptance step, then `run` refused because the target was dirty. The document records no evidence
that distinguishes a clean successful run from that refusal.

### F3 — REJECT: the required detector is missing

B8 requires `tests/corpus/M9.sh`, but the file is absent and its direct command exits 127. The
walkthrough’s “every non-zero exit” recommendation has no denominator, does not enumerate hard or
untriggerable surfaces, and has no mutation proof. A detector that was never run cannot establish
the claimed property.

### F4 — REJECT: the implementation still has silent and vacuous failure paths

The valid empty-task refusal and tampered-ledger mismatch are silent. Fresh ledger verification is
also silent on exit 6. Meanwhile fresh `status --json` exits 0 with `checked=0 total=0`. These are
exactly the user-visible and denominator failures B8 is supposed to close.

### F5 — REJECT: the independent full gate is red

The required verifier exited 6. A 14/16 stage result with a failed corpus stage cannot satisfy B8,
regardless of the 34/34 detector denominator, because nine timed-out detectors were treated as
failures and the mutation stage was skipped.

## Required changes for acceptance

1. Restore a literal `opus-walkthrough` backlog item, or explicitly bind the deliverable to B8 and
   state that substitution in the contract.
2. Fix every cited silent path: valid empty-task refusal, tampered-ledger mismatch, fresh ledger
   verification, and the partial environment-fault plan. Every non-zero exit must print a reason.
3. Add `tests/corpus/M9.sh`. Enumerate every refusable surface, include a non-zero denominator and
   per-surface outcomes, and fail when a surface cannot be triggered or checks zero inputs.
4. Mutation-test M9 by removing or corrupting each reason assertion; publish the mutation
   checked/total result and restore the mutation afterward.
5. Re-run the complete flow in a clean disposable repository with exact commands and captured
   stdout, stderr, exit codes, state setup, and tamper/restore evidence.
6. Re-run `FLEET_MUTANTS=0 bash verify.sh` to exit 0 with `0 failed`, and publish the final
   `16/16` stage result plus any detector denominator and mutation evidence.
