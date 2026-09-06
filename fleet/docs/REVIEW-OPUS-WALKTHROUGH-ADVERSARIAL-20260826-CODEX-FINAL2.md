# Adversarial review: opus walkthrough

Date: 2026-08-26

Verdict: **REJECT**

## Contract

The relevant backlog item is B8 in `handover/BACKLOG.md:92-106`. It requires the three reported
failure paths to be fixed, a mutation-tested `tests/corpus/M9.sh` with a published denominator, and
a green `verify.sh` run.

The deliverable claims that the end-to-end flow works, then reports two silent failure paths and a
third partial-output environment failure in `docs/delta.d/opus-walkthrough.md:3-22`.

## What I ran

All runs used the checked-in `keel/target/debug/fleet` binary, isolated temporary state, and a
temporary git repository. Git was used only to initialize the temporary fixture; the target
checkout was not inspected or modified with git.

| Command / scenario | Exit | Observed |
|---|---:|---|
| `fleet plan "add a --version flag to the cli"` | 0 | 499 bytes; plan and denominator printed. |
| `fleet plan "make me a sandwich"` | 7 | 154 bytes; refusal and three near matches printed. |
| `fleet run --task "add a --version flag" --repo <repo> --agent stub` without SOW | 7 | 290 bytes; actionable SOW refusal printed. |
| Invalid `fleet sow --task "add a --version flag"` | 7 | 806 bytes; missing citation and corrected template printed. |
| Valid SOW, `sow accept`, then `run` | 9, 0, 0 | SOW was emitted, accepted, and a real artifact was produced. |
| `fleet run --task "" --repo <repo> --agent stub` | 7 | **stdout 0 bytes, stderr 0 bytes**. The reported defect reproduces. |
| Tampered `fleet ledger verify` | 8 | **stdout 0 bytes, stderr 0 bytes**. The reported defect reproduces. |
| Restored `fleet ledger verify` | 0 | `verified checked=11 total=11` in the same run. |
| `env -u FLEET_STATE fleet plan "add a --version flag to the cli"` | 3 | stdout 55 bytes (`intent`, `agent`, `skills`), stderr 264 bytes with the environment reason. The reported partial output reproduces. |
| Fresh-state `fleet ledger verify` | 6 | **stdout 0 bytes, stderr 0 bytes**. Empty ledger is another silent non-zero path. |
| `bash tests/corpus/M9.sh` | 127 | `No such file or directory`; M9 is absent. |

The literal command shown in the deliverable, `fleet run --task ""`, is incomplete. Run literally,
it exits 7 with an actionable `--repo is required` message. The silent defect requires the full
invocation including `--repo` and `--agent`; the document must show that reproducible command.

## Independent verifier

I ran the required command exactly:

```text
FLEET_MUTANTS=0 bash verify.sh
```

The run reached the corpus and produced these real failure signals:

```text
  TIMEOUT A1.sh exceeded 30s -- treated as FAILED
  TIMEOUT A3.sh exceeded 30s -- treated as FAILED
  TIMEOUT A4.sh exceeded 30s -- treated as FAILED
  TIMEOUT A8.sh exceeded 30s -- treated as FAILED
  TIMEOUT B10.sh exceeded 30s -- treated as FAILED
  TIMEOUT C1.sh exceeded 30s -- treated as FAILED
  TIMEOUT C11.sh exceeded 30s -- treated as FAILED
  TIMEOUT C19.sh exceeded 30s -- treated as FAILED
  TIMEOUT C2.sh exceeded 30s -- treated as FAILED
  TIMEOUT C22.sh exceeded 30s -- treated as FAILED
  TIMEOUT C24.sh exceeded 30s -- treated as FAILED
  TIMEOUT C26.sh exceeded 30s -- treated as FAILED
  TIMEOUT C3.sh exceeded 30s -- treated as FAILED
  TIMEOUT C6.sh exceeded 30s -- treated as FAILED
  TIMEOUT C8.sh exceeded 30s -- treated as FAILED
  TIMEOUT C9.sh exceeded 30s -- treated as FAILED
  TIMEOUT S4.sh exceeded 30s -- treated as FAILED
  TIMEOUT S6.sh exceeded 30s -- treated as FAILED
  TIMEOUT S9.sh exceeded 30s -- treated as FAILED
  TIMEOUT T1.sh exceeded 30s -- treated as FAILED
  TIMEOUT T15.sh exceeded 30s -- treated as FAILED
  TIMEOUT T20.sh exceeded 30s -- treated as FAILED
  TIMEOUT T5.sh exceeded 30s -- treated as FAILED
  TIMEOUT T6.sh exceeded 30s -- treated as FAILED
DENOMINATOR checked=34 total=34 excluded=69 caught=26
  FAIL corpus (see var/verify.log)
-- 14 passed, 1 failed, 1 skipped (denominator: 16) --
VERIFY_EXIT=6
```

The corpus denominator is internally consistent: 34 checked + 69 excluded = 103 detector files
selected by `run.sh`; 26 of the 34 checked detectors were caught failures. The verifier therefore
does not support a green claim.

## Findings

1. **The documented reproduction is not executable as written.** `fleet run --task ""` omits the
   required repository and agent arguments and produces a non-silent usage refusal. This makes the
   evidence ambiguous and can cause a reviewer to test the wrong path. Document the complete
   invocation, temporary-state setup, and separate stdout/stderr byte counts.

2. **The B8 implementation acceptance is not met.** `tests/corpus/M9.sh` does not exist, its
   manifest entry does not exist, and the three primary defects still reproduce. Add M9, make it
   drive every refusable surface, assert a non-empty reason on every non-zero exit, publish
   `checked/total`, fail when `checked==0`, and add mutation coverage in both directions.

3. **The walkthrough misses a known empty-input failure.** `fleet ledger verify` on a fresh state
   returns exit 6 with no output. `ledger_rows(false)` deliberately returns the invariant exit
   before `ledger_verify` can print its success denominator (`keel/fleet/src/main.rs:2897-2904,
   2995-2999`). M9 must cover this path and the implementation must emit an actionable reason.

4. **The claimed verifier state is false for this checkout.** The required verifier ended red with
   26 caught corpus failures and exit 6. Do not mark B8 complete until the verifier itself exits 0
   with its final `passed/failed/skipped` denominator, and do not discard or relabel the 26 caught
   failures as skips.

## Required changes for REJECT

1. Emit a human-readable reason before returning from the empty-task, tampered-ledger, and fresh
   empty-ledger paths; preserve exit codes 7/8/6.
2. Reorder or redesign `plan` environment validation so an environment refusal does not emit a
   misleading partial plan, while still naming the required fix.
3. Add and integrity-seal `tests/corpus/M9.sh`; publish its checked/total denominator and mutation
   test it against both a genuine failure and a detector that has been weakened.
4. Rerun `FLEET_MUTANTS=0 bash verify.sh` and require a terminal exit 0 with no red stages. Update
   the walkthrough with the exact reproducible commands and observed counts only after that run.
