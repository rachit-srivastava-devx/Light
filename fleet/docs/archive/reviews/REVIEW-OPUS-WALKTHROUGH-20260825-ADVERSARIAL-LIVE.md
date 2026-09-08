# Adversarial review: opus-walkthrough

Date: 2026-08-25  
Deliverable: `docs/delta.d/opus-walkthrough.md`  
Contract source: `handover/BACKLOG.md`

## Verdict: REJECT

The walkthrough identifies real silent failure paths, but its primary command is
not reproducible as written, its end-to-end success claim has no executable
transcript and did not reproduce in the replay, its counts are incomplete, and
the required B8 detector/verification acceptance is not met.

There is no literal `opus-walkthrough` item in `handover/BACKLOG.md`. The only
matching contract is B8, lines 92-106, which names this document as evidence and
requires all three defects to be fixed, `tests/corpus/M9.sh` to exist with a
published denominator and mutation test, and `verify.sh` to be green. I used
that B8 block as the operative contract and record the naming discrepancy below.

## What was claimed

From `docs/delta.d/opus-walkthrough.md:3-22`:

1. The `plan` → SOW refusal → SOW acceptance → run → status → ledger tamper and
   restore flow works end to end.
2. `fleet run --task ""` exits 7 with zero bytes on both streams.
3. Tampered `fleet ledger verify` exits 8 with no output, while success prints
   `verified checked=12 total=12`.
4. Unset `FLEET_STATE` makes `fleet plan` print `intent:`, `agent:`, and
   `skills:` before an environment refusal.
5. These are “two defects” missed because the suite asserts exit codes only;
   the proposed detector should check every non-zero exit for a named reason.

## What I actually ran

All commands used the quoted repository path and the existing built binary at
`keel/target/release/fleet`. I did not run Git commands. Temporary state and a
temporary copied target were outside the repository.

### Direct claims and failure paths

| Command | Exit | stdout bytes | stderr bytes | Observation |
|---|---:|---:|---:|---|
| `env -u FLEET_STATE PATH="$PWD/keel/target/release:$PATH" fleet run --task ""` | 3 | 0 | 64 | Environment fault: `MISSING_FLEET_STATE`, not the claimed exit 7. |
| `FLEET_STATE=<state> ... fleet run --task ""` | 7 | 0 | 187 | Refuses because `--repo` is missing; prints usage. |
| `FLEET_STATE=<state> ... fleet run --task "" --repo "$PWD" --agent stub` | 7 | 0 | 0 | Reproduces the underlying silent empty-task refusal. |
| `env -u FLEET_STATE ... fleet plan "add a --version flag to the cli"` | 3 | 55 | 264 | Prints the three claimed fields, then reports the missing environment. |
| `FLEET_STATE=<fresh-state> ... fleet ledger verify` | 6 | 0 | 0 | Additional silent failure on an empty ledger. |
| `FLEET_STATE=<fresh-state> ... fleet status --json` | 0 | 452 | 0 | Vacuous success: `checked=0`, `total=0`, `empty=true`. |

For the tamper direction, I seeded a refusal receipt in a temporary state,
appended invalid data to `ledger/chain.jsonl`, and ran:

```text
fleet ledger verify                 -> exit 8, stdout 0, stderr 0
restore chain.jsonl
fleet ledger verify                 -> exit 0, stdout 27, stderr 0
                                      verified checked=5 total=5
```

This confirms the silent tamper defect, but it does not support the document's
unexplained historical `12/12` count.

### End-to-end replay

I copied the disposable target found at `/private/tmp/fleet-target.XwiU4f` to a
new temporary directory, used a new temporary `FLEET_STATE`, and ran the full
SOW sequence with the concrete multiline task from `README.md`.

| Step | Exit | Observation |
|---|---:|---|
| `fleet plan "add a --version flag to the cli"` | 0 | Rendered plan; `commands: 3 planned (denominator: 3)`. |
| `fleet run --task <task> --repo <target> --agent stub` before SOW | 7 | Actionable `SOW_NOT_ACCEPTED` refusal, 290 stderr bytes. |
| `fleet sow --task "add a --version flag"` | 7 | Actionable missing-citation refusal, 806 stderr bytes. |
| `fleet sow --task <complete task>` | 9 | `SOW_READY_AWAITING_REVIEW` with an ID. |
| `fleet sow accept --id <captured ID>` | 0 | `SOW_ACCEPTED` printed. |
| `fleet run --task <same task> --repo <target> --agent stub` after acceptance | 7 | Refused: target repo has uncommitted changes; no artifact was produced. |
| `fleet status` | 0 | Rendered a receipt report with one failed task. |
| `fleet ledger verify` | 0 | `verified checked=5 total=5`. |
| `fleet attest verify <artifact>` | 8 | No artifact ID existed because the run failed. |
| Tampered `fleet ledger verify` | 8 | No output. |
| Restored `fleet ledger verify` | 0 | `verified checked=5 total=5`. |

The walkthrough supplies no exact task, state directory, binary/version,
clean-target precondition, command exit codes, artifact ID, or status/tamper
transcript. The claimed successful run therefore cannot be independently
replayed from the deliverable, and the attempted replay did not reach an
artifact.

### Required independent verifier

Exact command:

```text
FLEET_MUTANTS=0 bash verify.sh
```

Terminal output:

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

Exit code: **6**.

`var/verify.log` reported `detector-integrity: 105 detectors match the
manifest (denominator: 105)`, then the corpus result:

```text
  TIMEOUT A1.sh exceeded 30s -- treated as FAILED
  TIMEOUT A3.sh exceeded 30s -- treated as FAILED
  TIMEOUT A4.sh exceeded 30s -- treated as FAILED
  TIMEOUT A8.sh exceeded 30s -- treated as FAILED
  TIMEOUT C1.sh exceeded 30s -- treated as FAILED
  TIMEOUT C11.sh exceeded 30s -- treated as FAILED
  TIMEOUT C2.sh exceeded 30s -- treated as FAILED
  TIMEOUT C22.sh exceeded 30s -- treated as FAILED
  TIMEOUT C26.sh exceeded 30s -- treated as FAILED
  TIMEOUT C3.sh exceeded 30s -- treated as FAILED
  TIMEOUT C6.sh exceeded 30s -- treated as FAILED
  TIMEOUT S6.sh exceeded 30s -- treated as FAILED
  TIMEOUT S9.sh exceeded 30s -- treated as FAILED
  TIMEOUT T1.sh exceeded 30s -- treated as FAILED
  TIMEOUT T15.sh exceeded 30s -- treated as FAILED
  TIMEOUT T5.sh exceeded 30s -- treated as FAILED
DENOMINATOR checked=34 total=34 excluded=69 caught=19
```

The corpus denominator is internally consistent for its scored population
(`34 + 69 = 103` after excluding `run.sh` and `_selftest.sh`), but it is not the
same population as the 105-file integrity denominator. The final gate is red;
it cannot satisfy B8's green-verifier criterion.

## Findings

### F1 — REJECT: the primary reproduction command is incomplete

`docs/delta.d/opus-walkthrough.md:11` cites `fleet run --task ""`, but the
literal command does not reach the empty-task branch. With state unset it fails
with environment exit 3; with state set it fails earlier on missing `--repo`
and prints 187 bytes of usage. The claimed 7/0/0 result requires the omitted
`--repo` and `--agent` arguments.

Required change: cite the complete command and preconditions, and distinguish
the argument-validation refusal from the valid empty-task refusal. Then fix the
valid empty-task branch to emit a named reason while preserving exit 7.

### F2 — REJECT: “works end to end” is not evidence-backed

The document gives a prose arrow sequence but no replayable transcript. The
accepted run in the disposable replay was refused because the target was dirty,
so no artifact or attestation was created. The document cannot establish that
the success path reached `status`, a 12-row ledger, or a successful attestation.

Required change: either remove the success claim or add a complete transcript
from a disposable clean target with exact setup, task, state, binary, every exit
code, artifact ID, status denominator, attestation result, tamper mutation, and
restore result.

### F3 — REJECT: arithmetic and coverage are incomplete

The document says “Two defects” at lines 8-9 but enumerates a third at lines
16-18. It publishes no denominator for the walkthrough: no number of commands
checked, no success/refusal total, no omitted hard cases, and no classification
of untriggerable cases. `checked=12 total=12` is only a ledger-row claim and is
not a walkthrough coverage denominator.

Required change: correct the count and publish a checkable `checked/total`
population for the complete flow, including the three failure paths and any
omitted empty-store/empty-ledger cases.

### F4 — REJECT: the suite explanation is false and M9 is absent

The claim at lines 8-9 that the suite “asserts exit codes” only is contradicted
by the acceptance output: `Q1 all 11 refusal surfaces are actionable
(denominator: 11)` passed, and the acceptance script checks that missing
`FLEET_STATE` output names the variable. Those checks do not cover these exact
three paths, but the blanket explanation is inaccurate.

`tests/corpus/M9.sh` is absent. The only non-review M9 reference is the B8
requirement itself. Therefore the proposed every-non-zero-exit detector has no
mechanized denominator and no mutation result. B8 explicitly requires the
detector, mutation testing, and a green verifier; the independent verifier is
red.

Required change: implement M9 over every reachable refusable surface, publish
the tested denominator, test both reason-present and reason-absent mutations,
and rerun the full verifier to exit 0. Correct the suite-coverage statement to
name what is and is not covered.

### F5 — Additional omitted vacuous-success case

On a fresh state, `fleet status --json` exits 0 and reports
`checked=0,total=0,empty=true`. The walkthrough does not classify this hard case,
although the project rules reject checks that measure nothing. A fresh
`fleet ledger verify` is also a silent non-zero failure (exit 6).

Required change: decide and document the empty-store/empty-ledger contract,
assert it, and include it in the detector's published surface denominator.

## Exact changes required for acceptance

1. Restore or explicitly name the missing `opus-walkthrough` backlog contract;
   if B8 is the contract, state that in the deliverable.
2. Correct the incomplete empty-task command and fix all three user-visible
   failure paths to emit named reasons.
3. Replace the unsupported end-to-end prose with a complete clean-target
   transcript, or withdraw the success claim.
4. Publish a complete walkthrough denominator and correct “two” to “three”.
5. Implement and mutation-test `tests/corpus/M9.sh`, then rerun
   `FLEET_MUTANTS=0 bash verify.sh` until the final exit is 0 with the full
   stage denominator.

