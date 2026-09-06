# Adversarial review: opus walkthrough

Date: 2026-08-26  
Deliverable: `docs/delta.d/opus-walkthrough.md`  
Contract: `handover/BACKLOG.md`, B8 — Every non-zero exit must print a reason

## Verdict

**REJECT**

The walkthrough correctly identifies three human-surface defects, and all three reproduce. The
contract is not met: the defects are not fixed, `tests/corpus/M9.sh` does not exist, and the required
independent verifier is red.

## What was claimed

The deliverable claims:

1. A complete `plan` → SOW refusal → SOW acceptance → `run` → `status` → ledger verification →
   tamper → restore flow works end to end.
2. `fleet run --task ""` exits 7 with zero output.
3. Tampered `fleet ledger verify` exits 8 with zero output.
4. `fleet plan` with `FLEET_STATE` unset prints `intent:`, `agent:`, and `skills:` before the
   environment fault.
5. A detector should assert that every non-zero exit prints at least one reason line.

The claimed defect count is arithmetically three: two numbered defects plus one third defect. The
walkthrough does not publish a command-by-command denominator or captured output for the claimed
full flow, so that positive claim is not independently auditable from the document.

## What I actually ran

I used the existing release binary at
`/Users/rachitsrivastava/youtube/Principal Engineering/fleet-rs/keel/target/release/fleet`, with
fresh `mktemp -d` state directories. I did not run git commands.

| Command / case | Exit | Captured output | Observation |
|---|---:|---:|---|
| `fleet plan "add a --version flag"` with state set | 0 | stdout 499 B, stderr 0 B | Plan rendered; it published `commands: 3 planned (denominator: 3)`. |
| `fleet plan ""` | 7 | stdout 0 B, stderr 41 B | Refusal names usage. |
| `fleet run --task "add a --version flag" --repo /tmp/fleet-target.XwiU4f --agent stub` without accepted SOW | 7 | stdout 0 B, stderr 290 B | Refusal names `SOW_NOT_ACCEPTED` and gives the SOW id/instructions. |
| `fleet sow --task ""` | 7 | stdout 0 B, stderr 43 B | Refusal says the task is empty. |
| Valid structured SOW task | 9 | stdout 1502 B, stderr 194 B | SOW generated; refusal included the missing citation and a corrected template. |
| `fleet sow accept --id fcea10dfc63d8f375525606973ef28bb52c875b0cc333ea687b5897fdd51b99f` | 0 | stdout 125 B, stderr 0 B | Acceptance recorded. |
| `fleet run --task "" --repo /tmp/fleet-target.XwiU4f --agent stub` | 7 | stdout 0 B, stderr 0 B | **Claim reproduced exactly.** |
| `env -u FLEET_STATE fleet plan "add a --version flag"` | 3 | stdout 55 B, stderr 264 B | **Claim reproduced exactly:** stdout contains `intent:`, `agent:`, `skills:` before the state fault. |
| `fleet ledger verify` on an intact chain | 0 | stdout 27 B, stderr 0 B | `verified checked=3 total=3`. |
| Same ledger after changing one JSON field | 8 | stdout 0 B, stderr 0 B | **Claim reproduced exactly.** No mismatch reason or denominator is emitted. |
| `fleet ledger verify` after restoring the ledger | 0 | stdout 27 B, stderr 0 B | `verified checked=3 total=3`. |

The accepted-SOW run could not complete against the available throwaway target: the binary refused
`/tmp/fleet-target.XwiU4f` as already dirty with exit 7 and 449 bytes of explanation. That means the
walkthrough's end-to-end success claim is not proven by this review; the SOW refusal/acceptance and
tamper/restore portions were exercised independently.

## Independent verifier

Command:

```text
FLEET_MUTANTS=0 bash verify.sh
```

Actual final output:

```text
  SKIPPED WITH A REASON mutants            (set FLEET_MUTANTS=1 - full pass ~24min)
  ok   attest-smoke              
  ok   pytest                    
  ok   detectors                 
  FAIL corpus                     (see var/verify.log)
-- 14 passed, 1 failed, 1 skipped (denominator: 16 stages) --
```

Exit code: **6**.

The corpus log publishes its own denominator:

```text
DENOMINATOR checked=34 total=34 excluded=69 caught=19
```

This is red evidence, not a green gate. The log also reports detector timeouts and
`M6: documented but missing: fleet arch`.

## Findings

### F1 — B8's three defects remain in the product

Failure → the three exact reproductions above still produce silent or partial human output.  
Cause → `run_with_evidence` records an empty-task refusal and returns at
`keel/fleet/src/main.rs:904-912` without an `eprintln!`; `ledger_verify` prints only after
`verify_rows` succeeds at `keel/fleet/src/main.rs:2995-2999`; `plan_command` prints three lines at
`keel/fleet/src/main.rs:2659-2661` before calling `state_dir()` at line 2664.  
Fix → emit a typed, actionable reason on every failure path, and validate/buffer environment state
before writing plan output.

### F2 — Required M9 detector is absent

`tests/corpus/M9.sh` is absent. Therefore the required mechanisation of every refusable surface,
published denominator, and mutation test does not exist. A receipt-only assertion is not enough:
the contract requires a terminal reason line as observed by the user.

### F3 — The required verifier is not green

`FLEET_MUTANTS=0 bash verify.sh` exited 6 with `14 passed, 1 failed, 1 skipped` over 16 stages.
The acceptance criterion explicitly requires `verify.sh` green, so this review cannot accept the
walkthrough as an accepted B8 slice.

## Exactly what must change for acceptance

1. Fix the empty-task `run` path so exit 7 writes at least one human-readable reason line naming
   `EMPTY_TASK` or equivalent.
2. Fix ledger verification failures so exit 8 writes a mismatch reason and publishes the checked
   denominator where it is known. Preserve the existing success line and `checked=... total=...`.
3. Make `fleet plan` validate `FLEET_STATE` before emitting `intent`, `agent`, or `skills`, or
   buffer all plan output until the environment check succeeds. The failure must still explain the
   environment fix.
4. Add `tests/corpus/M9.sh` covering every reachable refusable surface, with a non-zero denominator,
   explicit checked/total output, and assertions that each non-zero result includes a reason line.
   Include both a passing detector run and mutation tests that prove the detector turns red when a
   reason is removed.
5. Re-run the full `FLEET_MUTANTS=0 bash verify.sh` and publish the final exit code and summary only
   after the corpus stage and all 16 stages are green. The current `14/16` result is not acceptance
   evidence.
