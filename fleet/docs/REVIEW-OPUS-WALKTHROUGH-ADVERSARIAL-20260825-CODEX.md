# Adversarial review: opus walkthrough

Date: 2026-08-25  
Deliverable: `docs/delta.d/opus-walkthrough.md`  
Contract: the only literal backlog reference is B8 in `handover/BACKLOG.md:92-106`; no
`opus-walkthrough`-named item exists in the current backlog.

## Verdict

**REJECT**

The two silent failure observations are reproducible, but the claimed successful end-to-end
flow is not reproduced, the document contradicts itself on the number of defects, its claim about
the suite is stale, and the required B8 gate is not implemented or green.

## What was claimed

- A full `plan` → SOW → accepted `run` → `status` → ledger verification → tamper → restore flow
  works end to end.
- `fleet run --task ""` exits 7 silently.
- Tampered `fleet ledger verify` exits 8 silently.
- Unset-state `fleet plan` emits partial output before exit 3.
- The suite missed all three because it checked exit codes only.
- A detector should enforce a reason on every non-zero exit.

The document says “Two defects” but lists those two defects and then “A third, smaller” defect.
That is three failure cases, not two.

## What I actually ran

All runs used the explicit `keel/target/debug/fleet` binary, disposable `FLEET_STATE`, and a
disposable target materialized from the repository HEAD. No shell Git command was used.

| Command / case | Exit | Observed |
|---|---:|---|
| `fleet plan "add a --version flag to the cli"` | 0 | Plan printed; `commands: 3 planned (denominator: 3)`. |
| `fleet plan "make me a sandwich"` | 7 | Refusal printed three candidates. |
| Valid task `fleet run ... --agent stub` before SOW acceptance | 7 | Refusal named the SOW id and next commands. |
| `fleet sow --task "add a --version flag"` | 7 | Refusal named the missing evidence citation and printed a template. |
| Valid `fleet sow --task "$task"` | 9 | `SOW_READY_AWAITING_REVIEW`; an id was emitted. |
| `fleet sow accept --id <id>` | 0 | `SOW_ACCEPTED`. |
| Accepted `fleet run --task "$task" --repo <clean-target> --agent stub` | 6 | No artifact. The adapter reported: `agent stub exited without an fd-3 result`. |
| `fleet status` after that run | 0 | `3 of 3` receipts, but the task was **PENDING** with “run started; no terminal receipt”. |
| Untampered `fleet ledger verify` | 0 | `verified checked=8 total=8`. |
| Tampered `fleet ledger verify` | 8 | `stdout=0`, `stderr=0`. |
| Restored `fleet ledger verify` | 0 | `verified checked=8 total=8`. |
| `fleet run --task "" --repo <target> --agent stub` | 7 | `stdout=0`, `stderr=0`. |
| `env -u FLEET_STATE fleet plan "add a --version flag to the cli"` | 3 | `stdout=55`, `stderr=264`; stdout contained `intent:`, `agent:`, `skills:` before the environment fault. |

The successful artifact path therefore did not resolve in this review. The document’s historical
`checked=12 total=12` is not a measurement from this run and has no reproducible command, ledger
row count, or run transcript attached to it.

## Independent verifier

Command:

```text
FLEET_MUTANTS=0 bash verify.sh
```

Real final output:

```text
  FAIL corpus                     (see var/verify.log)
-- 13 passed, 2 failed, 1 skipped (denominator: 16 stages) --
```

The earlier stage output included:

```text
  FAIL swarm                      (see var/verify.log)
  SKIPPED WITH A REASON mutants            (set FLEET_MUTANTS=1 - full pass ~24min)
```

`var/verify.log` recorded `CC1 six concurrent dispatches on a cold store all succeed 1 of 6
failed`, plus timeout failures for corpus detectors including `A4.sh`, `C11.sh`, `C2.sh`,
`C22.sh`, `C6.sh`, `S6.sh`, `S9.sh`, `T1.sh`, and `T15.sh`. `tests/corpus/M9.sh` is absent.

## Findings

1. **REJECT: end-to-end success is unsupported.** The document presents the flow as working, but
   the current replay did not produce an artifact or terminal receipt. It also gives no commands,
   task text, exit codes, clean-state setup, or denominator that another operator can rerun.

2. **REJECT: the suite claim is false as written.** `tests/acceptance/p0.sh` currently asserts the
   unset-`FLEET_STATE` environment message (`I1`/`I2`), and the verifier reported those checks green.
   The suite may still miss the two zero-byte paths, but it did not miss all three for the stated
   reason.

3. **REJECT: B8 acceptance is unearned.** The backlog requires all three fixes, an M9 detector with
   a published denominator, mutation testing, and a green verifier. The two silent paths remain
   silent, M9 is absent, and the required verifier is red with exit 6.

4. **Finding: denominator and arithmetic are incomplete.** The document publishes `12 of 12` only
   as an asserted success-path example. It does not publish how many walkthrough commands/cases
   were executed, how many succeeded, or how many were blocked. The prose also counts two defects
   while recording three.

## Exactly what must change for acceptance

1. Make the walkthrough a reproducible transcript: include setup, exact commands, exit codes,
   stdout/stderr byte counts for failures, artifact/status/attestation evidence for success, and
   checked/total counts. Mark the flow BLOCKED if the accepted run cannot complete.
2. Correct the “two” versus “three” wording and narrow the suite statement to the paths it actually
   misses. Decide and document whether unset-state `plan` should fail before rendering partial
   output, then assert that behavior.
3. Fix the empty-task and tampered-ledger user messages, add `tests/corpus/M9.sh` with a non-zero
   denominator and both positive/negative mutation cases, and rerun until
   `FLEET_MUTANTS=0 bash verify.sh` exits 0 with its final summary captured.

