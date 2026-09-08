# Adversarial review: opus walkthrough

Date: 2026-08-26  
Deliverable: `docs/delta.d/opus-walkthrough.md`  
Reviewer: independent runtime review; no git commands run

## Verdict: REJECT

The document identifies real runtime defects, but it is not a reproducible walkthrough and its
first command is factually wrong as written. It also omits a fresh-state vacuous-success defect,
publishes no walkthrough denominator, and has no matching `opus-walkthrough` item in
`handover/BACKLOG.md`. The applicable contract is B8, which requires the three failures to be
fixed, an M9 detector with a denominator and mutation test, and a green verifier. None of those
remediations are present here.

## What was claimed

The deliverable says that a full plan/refusal/SOW/run/status/ledger/tamper/restore flow worked,
then reports:

1. `fleet run --task ""` exits 7 with zero bytes on both streams.
2. Tampered `fleet ledger verify` exits 8 without output, while success prints
   `verified checked=12 total=12`.
3. Prompted `fleet plan` with `FLEET_STATE` unset prints `intent:`, `agent:`, and `skills:` before
   the environment fault.

It attributes all three to a test suite that checks exit codes but not user-visible diagnostics.

## What I actually ran

All CLI runs used `keel/target/release/fleet` and isolated `mktemp -d` state directories. Byte
counts are stdout/stderr respectively.

| Command or scenario | Exit | Output | Observation |
|---|---:|---:|---|
| `fleet run --task ""` exactly as written | 7 | 0 / 187 | **Does not reproduce the claim.** It refuses earlier with `--repo is required` and usage. |
| `fleet run --task "" --repo "$PWD" --agent stub` | 7 | 0 / 0 | **Silent valid empty-task refusal reproduced.** |
| `env -u FLEET_STATE fleet plan "change the service"` | 3 | 55 / 264 | **Partial output reproduced:** intent, agent, skills precede the environment diagnostic. |
| `env -u FLEET_STATE fleet plan` | 7 | 0 / 41 | Bare `plan` only prints usage; it does not reproduce the document’s partial-output claim. |
| `fleet plan "make me a sandwich"` | 7 | 0 / 154 | Refuses and names 3 close candidates. |
| `fleet sow --task "add a --version flag"` | 7 | 0 / 806 | SOW refusal names the missing citation and prints a corrected template. |
| `fleet run` before SOW acceptance, with a complete task and current repo | 7 | 0 / 290 | Correct SOW refusal with an ID and recovery commands. |
| complete `fleet sow` | 9 | 1408 / 194 | SOW ready; an ID was emitted. |
| `fleet sow accept --id <emitted-id>` | 0 | 125 / 0 | Acceptance receipt emitted. |
| accepted `fleet run` against the shared checkout | 7 | 0 / 519 | Correctly refuses because the target repo has uncommitted changes; no successful run was observed. |
| `fleet status` after the refusal receipts | 0 | 2571 / 0 | Reports `2 of 2 tasks from receipts`; this is not the claimed successful end-to-end run. |
| clean copied ledger: `fleet ledger verify` | 0 | 27 / 0 | `verified checked=5 total=5`. The document’s `12 of 12` is not independently auditable. |
| tampered ledger: `fleet ledger verify` after replacing a row hash | 8 | 0 / 0 | **Silent mismatch reproduced.** |
| restored ledger: `fleet ledger verify` | 0 | 27 / 0 | `verified checked=5 total=5`; restoration works, but the failure has no reason. |

The tamper run changed a copied `ledger/chain.jsonl` hash to a 64-zero BLAKE3 value, ran verify,
then restored the copied chain. No repository files were changed.

## Arithmetic and omitted hard cases

- The walkthrough has no denominator for its 10-step flow, no per-step pass/fail table, no state
  fixture, and no tamper command. The lone historical `checked=12,total=12` cannot be checked by
  hand from this document.
- On a separate fresh state, `fleet ledger verify` returned exit 6 with 0/0 output.
- On that same fresh state, `fleet status --json` returned exit 0 with `checked=0,total=0,empty=true`.
  This is a vacuous success and violates the project law that a check examining zero inputs fails.
  The walkthrough does not test or disclose it.
- `tests/corpus/M9.sh` is missing, and no standalone `opus-walkthrough` acceptance test exists.
- `FLEET_MUTANTS=0` explicitly skips mutation testing; the document’s proposed detector has no
  mutation evidence.

## Independent verifier result

Command run exactly:

```text
FLEET_MUTANTS=0 bash verify.sh
```

Real terminal result, including red:

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
  SKIPPED WITH A REASON mutants            (set FLEET_MUTANTS=1 - full pass ~24min)
  ok   attest-smoke
  ok   pytest
  ok   detectors
  FAIL corpus                     (see var/verify.log)
-- 14 passed, 1 failed, 1 skipped (denominator: 16 stages) --
```

The corpus log ended with timeout failures and:

```text
DENOMINATOR checked=34 total=34 excluded=69 caught=27
```

Therefore the gate is red, not green. The corpus runner treats detector timeouts as failures; it
does not make them pass. The verifier’s terminal condition returns exit 6 for this one failed
stage.

## Required changes for acceptance

1. Add or restore an explicit `opus-walkthrough` backlog item, or state that B8 is the contract.
2. Rewrite the walkthrough with complete, copy-pasteable commands, isolated state setup, the
   repository fixture assumptions, the exact tamper edit, restore operation, exit codes, stream
   byte counts, and a published denominator for every step.
3. Correct the first finding to distinguish the non-reproducing shorthand from the valid empty-task
   command; retain the valid empty-task, tampered-ledger, and partial-plan findings.
4. Add the missing fresh-ledger/status cases and decide the empty-store contract. Do not represent
   `checked=0,total=0` as a clean success without an explicit cold-start decision.
5. Fix every non-zero path so it emits a human-readable reason, then add `tests/corpus/M9.sh` with
   a non-zero denominator, both refusal directions, and mutation evidence.
6. Rerun `FLEET_MUTANTS=0 bash verify.sh` to a final exit code of 0, and separately run the required
   mutation gate before claiming M9 is accepted.
