# Adversarial review: opus walkthrough

Verdict: **REJECT**

## What was claimed

`docs/delta.d/opus-walkthrough.md` claims that a complete `plan` → SOW → `run` → `status` →
ledger verification → tamper/restore walkthrough was run successfully, and reports three user-facing
failure-path defects:

1. Empty `run --task` exits 7 with no stdout or stderr.
2. Tampered `ledger verify` exits 8 with no output.
3. Unset `FLEET_STATE` makes `plan` print partial output before its environment refusal.

The document also gives `verified checked=12 total=12` as the successful ledger-verification result.

## Contract check

The named `opus-walkthrough` acceptance item is absent from `handover/BACKLOG.md`. The only matches
are the B8 reference at lines 94–96. There is therefore no acceptance contract to grade against.
That must be fixed before this deliverable can be considered complete.

## What I ran

All commands were run from the quoted repository path. The release binary build succeeded:

```text
cargo build --release                         exit 0
Finished `release` profile [optimized] ...
```

Using a fresh temporary `FLEET_STATE`:

```text
env -u FLEET_STATE fleet plan "add a --version flag"                  exit 3
stdout began: intent: implement a change / agent: builder / skills: rust
stderr then named: fleet: environment fault: FLEET_STATE is not set.

FLEET_STATE="$state" fleet run --task "" --repo . --agent stub       exit 7
stdout bytes: 0; stderr bytes: 0

FLEET_STATE="$state" fleet run --task "add a --version flag" ...     exit 7
refusal named: target repo has uncommitted changes
```

The SOW refusal was also reproduced. It exited 7 and printed the missing evidence-citation
question, source line, and corrected template. A valid SOW then exited 9, and `sow accept` exited 0.
The subsequent accepted `run` could not execute in this shared checkout because the product
correctly refused the dirty repository; I do not count that refusal as evidence that the claimed
successful run works.

The ledger checks were run independently. The successful check in my temporary state was:

```text
FLEET_STATE="$state" fleet ledger verify                         exit 0
verified checked=3 total=3
```

I tampered with a stored `exit_code` without recomputing its hash:

```text
FLEET_STATE="$state" fleet ledger verify                         exit 8
stdout bytes: 0; stderr bytes: 0
```

After restoring the original JSONL, verification returned exit 0 and printed `verified checked=3
total=3`. The document's `12 of 12` result is not reproducible from its undocumented walkthrough;
the only observed denominator here is 3 of 3.

Additional empty-input check:

```text
FLEET_STATE="$empty_state" fleet status --json                    exit 0
{"checked":0,"total":0,"empty":true,...}
```

This is a separate vacuous-success defect relevant to the repository's denominator law, although it
is not one of the three defects reported by the deliverable.

## Mandatory verifier

Command:

```text
FLEET_MUTANTS=0 bash verify.sh
```

Actual result: **exit 6**.

```text
  FAIL corpus                     (see var/verify.log)
-- 14 passed, 1 failed, 1 skipped (denominator: 16 stages) --
```

The failing corpus output contained these real timeout failures:

```text
TIMEOUT A3.sh A4.sh A8.sh C11.sh C2.sh C22.sh C26.sh C6.sh
TIMEOUT S6.sh S9.sh T1.sh T15.sh T5.sh
DENOMINATOR checked=34 total=34 excluded=69 caught=17
```

## Findings

### F1 — The three reported defects are real

Severity: confirmed.

The empty-task and tampered-ledger claims reproduce exactly, including silent non-zero exits. The
unset-state plan claim also reproduces. These are useful findings, not a reason to reject the
document by themselves.

### F2 — The end-to-end success claim is not auditable

Severity: blocking.

The document gives no exact commands, task text, state setup, tamper operation, receipt count, or
restore operation. In the current shared checkout the accepted run stops at the dirty-tree guard,
and the observed successful ledger denominator is 3, not 12. A reader cannot reproduce the claimed
successful path or check its arithmetic from this file.

### F3 — No denominator for the walkthrough coverage

Severity: blocking.

The file says “full flow” and “three” defects but does not publish the number of commands/cases
attempted, how many passed, or how many were blocked by the environment. The successful/blocked/
failed cases must sum to a stated denominator.

## Required changes for acceptance

1. Add the missing `opus-walkthrough` item and explicit acceptance criteria to `handover/BACKLOG.md`,
   or correct the review target to an existing item.
2. Rewrite the walkthrough with copy-pastable commands and captured exit code, stdout, and stderr for
   every step; include the exact tamper and restore operations.
3. Publish the arithmetic: total cases, successful cases, refused cases, blocked cases, and the
   receipt denominator. Explain why any historical `12 of 12` result cannot be reproduced, or supply
   the missing setup that produces it.
4. Separate historical evidence from this-run evidence. Do not state that the end-to-end flow works
   until it has completed on a clean repository and the resulting artifact/status/ledger outputs are
   shown.
5. Keep the three confirmed silent/partial-output findings, and link each to its exact reproducing
   command and observed bytes.

