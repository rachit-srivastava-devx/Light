# Walkthrough findings — driving fleet by hand as a user

Ran the full flow on 2026-08-24: `plan` → refused `plan` → `run` without a SOW → `sow` refused →
`sow` accepted → `run` → `status` → `ledger verify` → tamper → restore. The flow works end to end
and the pushbacks are genuinely good: the SOW refusal names the missing section, cites the line,
and writes the corrected template for you.

Two defects, both on the failure path, both invisible to the suite because the suite asserts exit
codes and **the exit codes are correct**:

1. `fleet run --task ""` — exit 7, **zero bytes on stdout and stderr**. A user sees nothing.
2. `fleet ledger verify` on a tampered chain — exit 8, **no output**. The success path prints
   `verified checked=12 total=12`; the failure path, which is the one that matters and the one the
   whole product is about, says nothing.

A third, smaller: `fleet plan` with `FLEET_STATE` unset prints `intent:`, `agent:` and `skills:`
and *then* hits the environment fault. Partial output before a refusal — check the environment
first, or print nothing.

This is the same class as D58/D59/D60. The code is right; the layer between the code and the
person using it is not covered. A detector should assert that **every non-zero exit writes at
least one line naming the reason** — that is mechanisable and would have caught all three.
