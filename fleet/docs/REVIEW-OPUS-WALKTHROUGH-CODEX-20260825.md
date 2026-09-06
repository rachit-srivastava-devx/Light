# Adversarial review: opus walkthrough

Date: 2026-08-25

Verdict: **REJECT**

## Contract check

`handover/BACKLOG.md` has no standalone `opus-walkthrough` item or acceptance criteria. The only
matching reference is B8, whose contract requires the three silent paths to be fixed, `M9.sh` to
exist with a published denominator, mutation testing, and a green `verify.sh`.

That contract mismatch is itself a finding. I used B8 as the nearest available contract.

## What the deliverable claims

- A complete `plan` → SOW → `run` → `status` → ledger verify → tamper → restore walkthrough works.
- The SOW refusal names the missing section, cites its line, and writes a corrected template.
- `fleet run --task ""` exits 7 with zero output.
- Tampered `fleet ledger verify` exits 8 with zero output.
- `fleet plan` with `FLEET_STATE` unset emits partial output before exit 3.
- A future detector should require a reason on every non-zero exit.

## Commands actually run

All runs used the built binary at `keel/target/release/fleet`, isolated temporary `FLEET_STATE`
directories, and the current checkout as `--repo .`. Output byte counts are stdout and stderr
separately.

| Surface | Exit | stdout | stderr | Observation |
|---|---:|---:|---:|---|
| Exact `fleet run --task ""` shorthand | 7 | 0 | 187 | Does not reproduce the claim: it stops earlier with visible `--repo is required`. |
| `fleet run --task "" --repo . --agent stub` | 7 | 0 | 0 | Reproduces the silent empty-task refusal. |
| `env -u FLEET_STATE fleet plan "diagnose fleet"` | 3 | 61 | 264 | Reproduces partial `intent`, `agent`, `skills` output before the environment error. |
| Non-empty run before SOW | 7 | 0 | 290 | Refusal names `SOW_NOT_ACCEPTED` and the accept command. |
| Empty ledger `ledger verify` | 6 | 0 | 0 | Additional silent invariant failure. |
| 12 valid ledger appends, then verify | 0 | 29 | 0 | `verified checked=12 total=12`. Denominator is present on success. |
| Tamper one row, then verify | 8 | 0 | 0 | Reproduces the silent tamper failure. |
| Restore the row, then verify | 0 | 29 | 0 | Returns to `verified checked=12 total=12`. |
| `fleet plan "make me a sandwich"` | 7 | 0 | 154 | Refusal is actionable and names three candidates. |
| `fleet sow --task "add a --version flag"` | 7 | 0 | 806 | Names the missing citation and prints the corrected template. |
| Filled SOW | 9 | 1408 | 194 | Emits `SOW_READY_AWAITING_REVIEW` and an acceptance command. |
| `fleet sow accept --id <id>` | 0 | 125 | 0 | SOW acceptance succeeds. |
| Accepted `run` against `--repo .` | 7 | 0 | 397 | Does not complete: current checkout is dirty and the product refuses it. |
| `fleet status` | 0 | 2483 | 0 | Renders an empty-store report with `0 of 0` tasks after the refused run. |

The SOW pushback claim is supported. The two primary silent-failure claims are supported only for
the complete invocations shown above; the exact shorthand in the deliverable is not a valid
reproduction of the silent path. The claimed full end-to-end run is not reproducible in this
checkout because the run stops at the dirty-repository guard, and the deliverable provides no
setup or transcript that isolates that precondition.

## Arithmetic and known-cheat checks

- The walkthrough reports “two defects” plus a “third, smaller” issue, but publishes no denominator
  for the proposed “every non-zero exit” detector.
- It does not classify the tested failure surfaces as checked, skipped, or untriggered.
- `tests/corpus/M9.sh` is absent. The corpus currently has 103 shell detectors; its runner excludes
  69 and checked 34, so a green-looking aggregate would not establish coverage of this request.
- The independent gate explicitly skipped mutation testing because it was run with
  `FLEET_MUTANTS=0`; no M9 mutation result exists.
- The implementation still has the observed behavior: empty-task handling returns exit 7 after
  writing a receipt but does not print a reason, and `ledger_verify` propagates errors before its
  success `println!`. The runtime evidence, not the green P0 exit code, is decisive here.

## Independent gate

Command executed exactly:

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
  SKIPPED WITH A REASON mutants  (set FLEET_MUTANTS=1 - full pass ~24min)
  ok   attest-smoke
  ok   pytest
  ok   detectors
  FAIL corpus                     (see var/verify.log)
-- 14 passed, 1 failed, 1 skipped (denominator: 16 stages) --
VERIFY_RC=6
```

`var/verify.log` records these corpus failures:

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

For comparison, the targeted acceptance command also ran:

```text
bash tests/acceptance/p0.sh
== 34 passed, 0 failed ==
P0_RC=0
```

Its empty-task assertion checks only exit 7 and redirects both streams to `/dev/null`; it does not
test the user-visible reason. Therefore P0 being green is not evidence against this finding.

## Findings

1. **Reject — the acceptance contract is missing.** There is no `opus-walkthrough` backlog item.
   The review target cannot be evaluated against the named contract until the item is added or the
   deliverable is explicitly assigned to B8.
2. **Reject — one central reproduction is wrong as written.** `fleet run --task ""` is visibly
   rejected for missing `--repo`; the silent defect requires `--repo . --agent stub`.
3. **Reject — evidence is not reproducible enough.** The deliverable gives no binary path,
   `FLEET_STATE`, repository precondition, command transcript, exit-code capture, or stream
   denominator. “Ran the full flow” cannot be independently replayed from the document.
4. **Reject — the product defect remains.** The valid empty-task and tampered-ledger cases still
   exit non-zero with zero user-visible bytes. P0 proves only the exit codes and receipt behavior.
5. **Reject — B8 is not met.** `M9.sh` is absent, mutation testing is not recorded, and the required
   verification command is red with exit 6.

## Exactly what must change

1. Add a real `opus-walkthrough` backlog item, or rename the review target’s contract reference to
   B8 and state that mapping explicitly.
2. Correct the empty-task reproduction to the complete command and include exact stdout/stderr,
   exit codes, isolated state setup, and the repository precondition for every flow step.
3. Publish a denominator for the tested refusal surfaces, including empty input, missing SOW,
   missing state, empty ledger, tampered ledger, and any surface that could not be triggered.
4. Fix the empty-task, tampered-ledger, and partial-plan user-visible behavior, then add
   `tests/corpus/M9.sh` to exercise every refusable surface and name the reason on each non-zero exit.
5. Mutation-test M9, publish its checked/total result, rerun `FLEET_MUTANTS=0 bash verify.sh`, and
   do not claim acceptance until the final exit code is 0.
