# Adversarial review — opus walkthrough

Date: 2026-08-25  
Verdict: **REJECT**

## Contract reviewed

The deliverable claims a hand-driven walkthrough found three failure-path defects and proposes a
detector. The relevant acceptance contract is B8 in `handover/BACKLOG.md`: fix all three, add
`tests/corpus/M9.sh` with a published denominator, mutation-test it, and make `verify.sh` green.
There is no separate `opus-walkthrough` item in `handover/BACKLOG.md`; `opus-walkthrough` appears
there only as the B8 evidence filename. That naming mismatch must be resolved before the item can
be graded independently.

## What was claimed

- The full `plan` → `run` → `sow` → `status` → ledger tamper/restore flow works end to end.
- A valid empty-task run exits 7 with zero bytes on stdout and stderr.
- A tampered ledger verification exits 8 with no output.
- A prompted plan with `FLEET_STATE` unset prints three plan lines before an environment fault.
- A detector should require every non-zero exit to emit a reason.

## What I ran

All commands used the existing release binary at
`keel/target/release/fleet`; temporary state was created with `mktemp -d` and removed after the
test. No git command was run.

| Check | Exit | stdout | stderr | Observation |
|---|---:|---:|---:|---|
| `fleet run --task ""` exactly as written | 7 | 0 bytes | 187 bytes | Prints `--repo is required`; the deliverable's exact shorthand does not reproduce its zero-byte claim. |
| `fleet run --task "" --repo . --agent stub` | 7 | 0 bytes | 0 bytes | Reproduces the substantive silent empty-task defect. A refusal receipt is written, but the user receives no reason. |
| `fleet ledger verify` on an untampered one-row chain | 0 | 27 bytes | 0 bytes | `verified checked=1 total=1`. |
| Same command after replacing the row hash with zeros | 8 | 0 bytes | 0 bytes | Reproduces the silent tampered-ledger defect. |
| `fleet plan "change the service"` with `FLEET_STATE` unset | 3 | 55 bytes | 264 bytes | Prints `intent: implement a change`, `agent: builder`, `skills: rust`, then reports the environment fault. |
| `fleet plan` exactly as written with `FLEET_STATE` unset | 7 | 0 bytes | 41 bytes | Prints only the usage error because the prompt is missing; the walkthrough omits the prompt needed to reproduce its partial-output claim. |

The successful ledger count I could reproduce was `1 of 1`, not the document's unaccompanied
`12 of 12` example. The walkthrough gives no commands for creating the 12-row state or for the
tamper operation, so that number is not independently auditable.

## Acceptance checks

- `tests/corpus/M9.sh`: **ABSENT**. No M9 denominator is published.
- Mutation evidence for M9: **ABSENT**.
- Arithmetic: the prose's “two defects” plus “a third” is three observations, but it publishes
  no denominator for the walkthrough flow or for the refusable surfaces required by B8.
- `FLEET_MUTANTS=0 bash verify.sh`: **exit 6**.

Real verifier result:

```text
ok   fmt
ok   clippy -D warn
ok   unit tests
ok   acceptance builds
ok   cargo-deny
ok   cargo-audit
ok   secrets
ok   acceptance
ok   readme
FAIL swarm (CC1: six concurrent dispatches on a cold store all succeed — 1 of 6 failed)
ok   policy
SKIPPED WITH A REASON mutants (FLEET_MUTANTS=0)
ok   attest-smoke
ok   pytest
ok   detectors
FAIL corpus
-- 13 passed, 2 failed, 1 skipped (denominator: 16 stages) --
```

The corpus log also recorded timed-out detectors treated as failures, including A4, C2, C6, C11,
C22, S6, S9, T1, and T15. Therefore the gate is not green, even aside from the missing M9.

## Findings

1. **The deliverable is not a reproducible user test.** The exact `fleet run --task ""` and bare
   `fleet plan` commands do not produce the reported observations. The report needs complete
   invocations, state setup, tamper command, and expected byte counts.
2. **The two core silent failures are real, but they remain unfixed.** The valid empty-task command
   and tampered-ledger command both return non-zero with no user-facing reason.
3. **The proposed detector is not implemented.** M9 is absent, has no denominator, and has no
   mutation-test evidence. A prose recommendation is not an acceptance result.
4. **The required gate is red.** `verify.sh` failed swarm and corpus, with a published final
   denominator of 16 stages; this cannot satisfy B8.

## Required changes for acceptance

1. Emit a reason on stderr for the valid empty-task refusal while preserving the refusal receipt.
2. Emit a reason on tampered-ledger verification failure, including enough context to identify the
   mismatch without claiming an unverified success denominator.
3. Preflight `FLEET_STATE` before plan output, or otherwise ensure the partial-output path is a
   deliberate, tested result with a reason.
4. Add `tests/corpus/M9.sh`; enumerate every refusable surface, publish `checked/total`, fail on
   zero inputs, and assert a reason on every non-zero exit.
5. Mutation-test M9 in both directions, then rerun `FLEET_MUTANTS=0 bash verify.sh` to a final
   exit 0 with the complete 16-stage denominator.
6. Correct the walkthrough to use reproducible full commands and either reproduce `12/12` or
   remove it. Resolve the missing `opus-walkthrough` backlog item name before claiming acceptance.
