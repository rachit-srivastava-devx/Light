# Adversarial review: opus-walkthrough

Review ID: `OPUS-WALKTHROUGH-20260826-ADVERSARIAL-PRIMARY`

Reviewed: `docs/delta.d/opus-walkthrough.md`. No Git command was run. No project
file other than this review was created or edited.

## Verdict

**REJECT**

The document records real user-facing defects, but it is not acceptance evidence
for the named item. There is no literal `opus-walkthrough` item in
`handover/BACKLOG.md`. The nearest binding contract is B8 at
`handover/BACKLOG.md:94-106`, and B8 remains unchecked: the three defects must be
fixed, `tests/corpus/M9.sh` must exist with a denominator and mutation coverage,
and `FLEET_MUTANTS=0 bash verify.sh` must be green.

## What was claimed

From `docs/delta.d/opus-walkthrough.md`:

- A full `plan` -> refusal -> `run` without SOW -> SOW refusal -> SOW acceptance
  -> `run` -> `status` -> `ledger verify` -> tamper -> restore flow works end to
  end (`:3-6`).
- `fleet run --task ""` returns exit 7 with zero bytes on both streams (`:11`).
- Tampered `fleet ledger verify` returns exit 8 with no output (`:12-14`).
- Unset-state `fleet plan` emits `intent:`, `agent:`, and `skills:` before its
  exit-3 environment fault (`:16-18`).
- A detector checking every non-zero exit should have caught all three (`:20-22`).

The document publishes no denominator for the walkthrough or for the proposed
detector. Its `checked=12 total=12` text is a historical success-path ledger
count, not walkthrough coverage.

## Commands actually run

The shell had no `fleet` executable on `PATH`, so the checkout binary was used:

```text
BIN="/Users/rachitsrivastava/youtube/Principal Engineering/fleet-rs/keel/target/release/fleet"
```

Temporary state directories were used. `stdout/stderr` below are byte counts.
All 22 invocations returned; this is an execution denominator, not a pass rate.

| Command case | Exit | stdout/stderr | Result |
|---|---:|---:|---|
| `FLEET_STATE=<state> fleet run --task ""` | 7 | 0/187 | Does not reproduce the claim; it reports `--repo is required`. |
| `FLEET_STATE=<state> fleet run --task "" --repo "$PWD" --agent stub` | 7 | 0/0 | Silent empty-task refusal reproduced. |
| `FLEET_STATE=<state> fleet plan "add a --version flag"` | 0 | 499/0 | Plan printed; `commands: 3 planned (denominator: 3)`. |
| `FLEET_STATE=<state> fleet plan "make me a sandwich"` | 7 | 0/154 | Refusal named 3 candidates. |
| `env -u FLEET_STATE fleet plan "add a --version flag"` | 3 | 55/264 | The three plan fields printed before the environment fault. |
| `env -u FLEET_STATE fleet plan` | 7 | 0/41 | Usage refusal. |
| `FLEET_STATE=<state> fleet run --task "add a --version flag" --repo "$PWD" --agent stub` before SOW | 7 | 0/290 | Actionable `SOW_NOT_ACCEPTED` refusal. |
| `FLEET_STATE=<state> fleet sow --task "fix it"` | 7 | 0/806 | SOW refusal named the missing citation and recovery template. |
| `FLEET_STATE=<state> fleet sow --task "add a --version flag"` | 7 | 0/806 | Same SOW refusal; this shorthand task was not accepted. |
| Same state, run after no SOW acceptance | 7 | 0/290 | `SOW_NOT_ACCEPTED`; no run. |
| `FLEET_STATE=<state> fleet status` | 0 | 2660/0 | Reported 3 of 3 receipt tasks as failed. |
| `FLEET_STATE=<state> fleet status --json` | 0 | 1431/0 | `checked=3 total=3`; denominator present for this non-empty state. |
| `FLEET_STATE=<state> fleet ledger verify` before tamper | 0 | 27/0 | `verified checked=5 total=5`. |
| Same ledger after changing `seq: 0` to `seq: 9` | 8 | 0/0 | Silent tampered-chain mismatch reproduced. |
| Restored ledger, `fleet ledger verify` | 0 | 27/0 | `verified checked=5 total=5`. |
| Fresh-state `fleet ledger verify` | 6 | 0/0 | Additional silent non-zero verification path. |
| Fresh-state `fleet status --json` | 0 | 452/0 | Vacuous green: `checked=0 total=0 empty=true`. |

### Positive SOW/run attempt

I also supplied the complete SOW sections shown by the refusal template:

| Command case | Exit | stdout/stderr | Result |
|---|---:|---:|---|
| `fleet sow --task <complete task with leaves/challenges/citations/alternatives/estimates/edge-cases>` | 9 | 1502/194 | `SOW_READY_AWAITING_REVIEW`; an ID was emitted. |
| `fleet sow accept --id <emitted ID>` | 0 | 125/0 | `SOW_ACCEPTED`. |
| `fleet run --task <same task> --repo "$PWD" --agent stub` | 7 | 0/519 | Refused because the shared target repository had uncommitted changes; no artifact or successful run was observed. |
| `fleet status --json` in the positive-flow state | 0 | 452/0 | `checked=0 total=0 empty=true`. |
| `fleet ledger verify` in the positive-flow state | 0 | 27/0 | `verified checked=3 total=3`. |

## Independent verifier

Exact command run:

```text
$ FLEET_MUTANTS=0 bash verify.sh
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
exit=6
```

The red corpus log reported:

```text
detector-integrity: 105 detectors match the manifest (denominator: 105)
TIMEOUT ... -- treated as FAILED
DENOMINATOR checked=34 total=34 excluded=69 caught=27
```

There were 25 timeout-labelled detector failures in the log. The verifier result
is not green and cannot satisfy B8.

## Findings

1. **The contract named by the request is absent.** `handover/BACKLOG.md` has no
   `opus-walkthrough` heading. B8 only references the document and remains `[ ]`.
   This makes the deliverable impossible to grade against the requested item and
   leaves the required fixes unspecified unless B8 is treated as the contract.

2. **The primary empty-task reproduction is incomplete as written.** The exact
   `fleet run --task ""` form exits 7 with 187 bytes of usage/error output. The
   silent behavior requires `--repo` and `--agent`, which the document omits.
   The underlying silent refusal is real, but the cited command does not prove it.

3. **The end-to-end success claim was not reproduced.** A complete SOW was
   created and accepted, but the accepted run exited 7 on the dirty shared
   checkout. The document supplies no exact binary, state directory, target repo,
   SOW text, clean-tree precondition, artifact ID, or successful run output. A
   historical assertion cannot establish a live end-to-end flow.

4. **The walkthrough has no denominator.** The document reports three defects,
   but does not state how many commands/surfaces were exercised, how many passed,
   how many failed, or which untriggerable/hard cases were excluded. The ledger's
   `12/12` is not that denominator. The fresh ledger verify (`6`, silent) and
   fresh status (`0`, `checked=0 total=0`) demonstrate omitted edge cases.

5. **The required detector is absent.** `tests/corpus/M9.sh` does not exist.
   `tests/corpus/run.sh` publishes its own `checked=34 total=34` corpus result,
   but that does not prove M9's every-refusable-surface property. No M9 mutation
   result exists.

6. **The required gate is red.** `FLEET_MUTANTS=0 bash verify.sh` returned exit
   6 with 14/16 stages passed, one failed, and one skipped. The failing corpus
   stage recorded 34/34 checked and 27 caught failures, including timeout-driven
   failures. A green claim would be false.

## Exactly what must change before acceptance

1. Restore a literal `opus-walkthrough` item in `handover/BACKLOG.md`, or
   explicitly rename/bind this deliverable to B8 so one contract governs it.
2. Replace the historical prose with a reproducible transcript: exact checkout
   binary, quoted state path, clean target-repository setup, complete SOW input,
   every command, exit code, stdout/stderr result, artifact ID, and a separate
   hand-checkable walkthrough denominator.
3. Correct the empty-task command to include the required `--repo` and `--agent`
   arguments, while retaining the observed fact that the valid empty-task refusal
   is silent.
4. Fix all three B8 user-facing paths: valid empty-task refusal, tampered-ledger
   mismatch, and unset-state plan. Every non-zero refusable surface must emit a
   line naming its reason, including the fresh-ledger case if M9 exercises it.
5. Add `tests/corpus/M9.sh` with a published non-zero-surface denominator and
   explicit classification of hard/untriggerable cases. Mutation-test both the
   positive and negative directions, then rerun verifier integrity.
6. Rerun `FLEET_MUTANTS=0 bash verify.sh` to a completed exit-0 result and paste
   the real output, including its stage denominator and any skipped stage reason.
