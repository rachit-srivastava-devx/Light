# Adversarial review: opus-walkthrough

Review ID: `OPUS-WALKTHROUGH-ADVERSARIAL-20260825-CODEX-FINAL`

Deliverable: `docs/delta.d/opus-walkthrough.md`

Contract used: `handover/BACKLOG.md` B8, the only backlog item that names this
deliverable. There is no standalone `opus-walkthrough` heading in the current
backlog. B8 is unchecked and requires the three fixes, `tests/corpus/M9.sh`
with a published denominator, mutation testing, and a green `verify.sh`.

## What was claimed

The walkthrough says a complete `plan` -> refusal -> `run` without SOW ->
refused/accepted SOW -> `run` -> `status` -> ledger verification -> tamper ->
restore flow works. It reports three user-visible failures:

1. `fleet run --task ""` exits 7 silently.
2. Tampered `fleet ledger verify` exits 8 silently.
3. `fleet plan` with `FLEET_STATE` unset prints plan fields before exit 3.

It proposes a detector covering every non-zero exit, but gives no command
denominator or classification of omitted surfaces.

## What I actually ran

All runs used the release binary at `keel/target/release/fleet`, with temporary
`FLEET_STATE` directories. No Git command was run.

| Case | Exit | stdout | stderr | Observation |
|---|---:|---:|---:|---|
| `fleet run --task ""` as written | 7 | 0 bytes | 187 bytes | Does not reproduce the claim; the command is incomplete and prints `--repo is required`. |
| `fleet run --task "" --repo "$PWD" --agent stub` | 7 | 0 bytes | 0 bytes | Reproduces the silent empty-task refusal, although the required arguments are absent from the walkthrough. |
| `env -u FLEET_STATE fleet plan` | 7 | 0 bytes | 41 bytes | Usage refusal, not the claimed partial plan. |
| `env -u FLEET_STATE fleet plan "add a --version flag"` | 3 | 55 bytes | 264 bytes | Reproduces partial `intent/agent/skills` output before the environment fault. |
| fresh-state `fleet ledger verify` | 6 | 0 bytes | 0 bytes | Additional silent non-zero path omitted by the walkthrough. |
| two-row ledger, valid `fleet ledger verify` | 0 | `verified checked=2 total=2` | 0 bytes | Success path includes a denominator. |
| same ledger after body tamper, `fleet ledger verify` | 8 | 0 bytes | 0 bytes | Reproduces the silent mismatch. |
| restored ledger, `fleet ledger verify` | 0 | `verified checked=2 total=2` | 0 bytes | Restore succeeds. |

I also drove the claimed sequence. `plan` succeeded with a 3-command plan;
unknown intent refused with exit 7; run without SOW refused with exit 7;
vague SOW refused with exit 7; valid SOW returned exit 9; SOW acceptance
returned exit 0; the subsequent run returned exit 7 because this shared target
repository has uncommitted changes; `status` returned exit 0 and reported
`1 of 1 tasks`; ledger verification returned exit 0 with `checked=5 total=5`.
Thus the full successful run claim is not reproducible from this checkout: the
walkthrough supplies neither a clean-target precondition nor exact setup and
restore commands.

The required detector is absent:

```text
M9_EXISTS=no
bash: tests/corpus/M9.sh: No such file or directory
M9_COMMAND_EXIT=127
```

## Arithmetic and hard-case audit

The prose's “two defects” plus “a third” is three observed cases, but it is not
a published `checked/total` denominator. It does not say how many refusal
surfaces were enumerated, how many were exercised, or which were untriggerable.
The `checked=12 total=12` success text is asserted without the commands that
created the 12 ledger rows and is not a walkthrough-coverage denominator.

The fresh empty ledger, missing `--repo`, missing prompt, unknown intent,
dirty-target refusal, and missing-state usage path are hard cases that must be
classified separately from the three reported defects. At least one of them
(fresh ledger) is silently non-zero and is missing from the proposed coverage.

## Independent verifier

Command:

```text
FLEET_MUTANTS=0 bash verify.sh
```

Result: exit 6 (the script's failure branch), with the real final output:

```text
  FAIL corpus                     (see var/verify.log)
-- 14 passed, 1 failed, 1 skipped (denominator: 16 stages) --
```

The failed corpus log includes:

```text
M2: 33942 files in the tree (>15000). Build artifacts are almost certainly inside the repo;
    set CARGO_TARGET_DIR outside it. Detectors walk the tree and will stall (D30).
...
TIMEOUT C6.sh exceeded 30s -- treated as FAILED
detector timed out
...
TIMEOUT S4.sh exceeded 30s -- treated as FAILED
detector timed out
...
TIMEOUT T6.sh exceeded 30s -- treated as FAILED
detector timed out
DENOMINATOR checked=34 total=34 excluded=69 caught=23
```

The verifier also explicitly skipped mutation testing because `FLEET_MUTANTS=0`:
`SKIPPED WITH A REASON mutants (set FLEET_MUTANTS=1 - full pass ~24min)`.
Therefore this is not evidence for B8's mutation-tested or green-gate criteria.

## Verdict: REJECT

The walkthrough identifies real silent failure behavior, but it is not
acceptance-complete and one literal command does not support its stated result.
The binding B8 contract is unmet: M9 does not exist, mutation testing is not
shown, and the independent verifier is red.

## Required changes

1. Restore an explicit `opus-walkthrough` contract item, or explicitly bind the
   deliverable to B8 and keep the acceptance mapping unambiguous.
2. Rewrite the walkthrough as a reproducible transcript: exact binary/setup,
   quoted paths, temporary state, required `--repo` and `--agent`, clean-target
   precondition, tamper command, restore command, exit codes, and both stream
   outputs. Correct the incomplete empty-task command and distinguish it from
   the actual silent complete-argument invocation.
3. Publish a hand-checkable walkthrough denominator and a separate denominator
   for all refusable surfaces. Enumerate and classify omitted, unavailable,
   environment-fault, refusal, mismatch, and success cases; zero checked cases
   must fail rather than pass vacuously.
4. Fix the three B8 paths, add `tests/corpus/M9.sh`, publish its checked/total
   result, and mutation-test the detector in both directions without allowing
   it to match its own documentation.
5. Rerun `FLEET_MUTANTS=0 bash verify.sh` on a quiet, correctly pruned tree and
   record the final exit code, every failed stage, and the final denominator;
   B8 cannot be accepted until that result is green.
