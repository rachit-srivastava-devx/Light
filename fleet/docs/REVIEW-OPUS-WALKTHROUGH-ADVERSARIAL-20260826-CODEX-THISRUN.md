# Adversarial review: opus walkthrough

Date: 2026-08-26

## Claimed

`docs/delta.d/opus-walkthrough.md` claims that the 2026-08-24 hand-driven flow works end to
end, and records three user-facing defects: empty-task refusal is silent, tampered-ledger
verification is silent, and `plan` emits partial output before an unset-`FLEET_STATE` environment
fault. It proposes a detector requiring every non-zero exit to print a reason.

The governing contract is B8 in `handover/BACKLOG.md`: fix all three paths; add
`tests/corpus/M9.sh` with a published denominator; mutation-test it; and have `verify.sh` green.

## Commands run and results

All commands used the real `keel/target/debug/fleet` binary. Disposable state directories were
created with `mktemp -d`; no repository `git` command was run.

| Command / scenario | Exit | Observed |
|---|---:|---|
| `FLEET_STATE=<tmp> fleet run --task "" --repo /tmp/fleet-target.XwiU4f --agent stub` | 7 | stdout 0 bytes, stderr 0 bytes; 2 receipt files were nevertheless written |
| Valid run attempt with `FLEET_SOW_BYPASS=1` on the existing fixture | 7 | Refused because the fixture was already dirty; stderr named the prerequisite |
| `FLEET_STATE=<tmp> fleet ledger verify` on the resulting chain | 0 | `verified checked=2 total=2` |
| Same ledger after changing the final event to `tampered-event` | 8 | stdout 0 bytes, stderr 0 bytes |
| Restored ledger `fleet ledger verify` | 0 | `verified checked=2 total=2` |
| `env -u FLEET_STATE fleet plan "add a --version flag"` | 3 | stdout 55 bytes: `intent`, `agent`, `skills`; stderr 264 bytes with the environment fault |
| Clean-copy `fleet run` before SOW acceptance | 7 | stderr 290 bytes naming `SOW_NOT_ACCEPTED` and the SOW commands |
| Vague `fleet sow --task "make it better"` | 7 | stderr 806 bytes with clarifying question and corrected template |
| Complete SOW creation | 9 | stdout 1408 bytes, stderr 194 bytes; emitted `SOW_READY_AWAITING_REVIEW` |
| `fleet sow accept --id <id>` | 0 | `SOW_ACCEPTED` |
| Clean-copy run after acceptance | 0 | artifact and oracle output; final `artifact=<64 hex chars>` |
| `fleet status` | 0 | `FLEET STATUS — 2 of 2 tasks from receipts` |
| `fleet ledger verify` after the positive flow | 0 | `verified checked=10 total=10` |
| `bash tests/acceptance/p0.sh` | 0 | `34 passed, 0 failed`; explicit `P0_EXIT=0` |

The positive flow therefore works in a clean disposable copy. The three defects recorded by the
deliverable also reproduce exactly. The source paths explain why: `drive_run` delegates an empty
task to the inner operation, but that path still exits silently; `ledger_verify` only prints after
`verify_rows` succeeds; and `plan_command` prints its first three lines before calling
`state_dir()`.

## Acceptance audit

- `tests/corpus/M9.sh` is absent. The corpus contains 105 shell detectors, but no M9 detector.
- The deliverable publishes no M9 `checked/total` denominator.
- No M9 mutation result is recorded. `FLEET_MUTANTS=0` explicitly skips the mutation stage.
- The deliverable says “the code is right” while its three live user paths are demonstrably not
  right at the CLI boundary.
- The positive walkthrough did not publish its own call denominator; its only count is the live
  ledger result (`10/10`), not coverage of the required refusable surfaces.

## Mandatory independent gate

Ran exactly:

```text
FLEET_MUTANTS=0 bash verify.sh
```

The first invocation produced this real red output before the corpus stage:

```text
  FAIL swarm                      (see var/verify.log)
  SKIPPED WITH A REASON mutants            (set FLEET_MUTANTS=1 - full pass ~24min)
  .... corpus
```

The log for that invocation included:

```text
  FAIL CC1 six concurrent dispatches on a cold store all succeed   1 of 6 failed
  TIMEOUT A1.sh exceeded 30s -- treated as FAILED
```

A second exact invocation was retained to completion rather than inferred from partial output. It
returned exit 6 with:

```text
  FAIL corpus                     (see var/verify.log)
-- 14 passed, 1 failed, 1 skipped (denominator: 16 stages) --
```

That run reached `swarm` green and skipped `mutants`; the corpus stage was the one failed stage.
Therefore `verify.sh` was not green. The earlier concurrent run also showed `swarm` red (`1 of 6`)
and an `A1.sh` timeout, so the checkout does not provide stable green evidence either.

## Verdict: REJECT

This is a useful defect report, but it does not satisfy its acceptance contract. The named defects
are still present, M9 is missing, the denominator and mutation evidence are missing, and the
mandatory gate has red/unverified evidence.

## Required changes for re-review

1. Preserve exit codes 7/8/3, but make each of the three paths emit at least one actionable
   reason, without leaking partial plan output before the environment check.
2. Add `tests/corpus/M9.sh`. Drive every refusable surface, assert non-zero exit plus a reason,
   publish `checked/total`, and fail if the checked set is empty. Do not count a surface that was
   not actually triggered.
3. Mutation-test M9 and show the detector goes red when each output guard is removed; restore the
   guard and record the real mutation denominator and result.
4. Rerun the full exact gate to a terminating final summary and exit code. Resolve every red item,
   including any corpus timeout or concurrent-dispatch failure, before claiming `verify.sh` green.
5. Update the walkthrough with the post-fix command output and denominators. Do not replace
   missing measurements with zero.
