# Adversarial review: opus walkthrough

Date: 2026-08-26
Reviewer: Codex, independent runtime review
Verdict: REJECT

## Contract under review

The named `opus-walkthrough` item is not present as a heading in the current
`handover/BACKLOG.md`. The only backlog contract that names this deliverable is
B8 (`handover/BACKLOG.md:92-106`): fix all three non-zero-output defects, add
`tests/corpus/M9.sh` with a published denominator, mutation-test it, and leave
`FLEET_MUTANTS=0 bash verify.sh` green.

## What was claimed

`docs/delta.d/opus-walkthrough.md` claims that a complete plan/SOW/run/status/
ledger/tamper/restore flow worked end to end, and records three user-facing
failure-path defects:

1. An empty task returns exit 7 with no stdout or stderr.
2. A tampered ledger returns exit 8 with no output.
3. An unset `FLEET_STATE` plan prints `intent:`, `agent:`, and `skills:` before
   the environment failure.

It proposes a detector for every non-zero exit, but it does not provide a
detector, denominator, mutation result, or verifier result.

## Commands actually run

All runtime checks used the checked-in release binary first and the verifier-built
`keel/target/debug/fleet` for the final pass. Temporary state copies were used
for ledger tampering; the shared checkout was not changed by those experiments.

| Command / scenario | Exit | stdout bytes | stderr bytes | Observation |
|---|---:|---:|---:|---|
| `env -u FLEET_STATE keel/target/release/fleet run --task ""` | 3 | 0 | 64 | The abbreviated command refuses earlier because state is unset. |
| `FLEET_STATE=<tmp> fleet run --task ""` | 7 | 0 | 187 | Missing `--repo` is reported. |
| `FLEET_STATE=<tmp> fleet run --task "" --repo "$PWD" --agent stub` | 7 | 0 | 0 | Exact silent empty-task defect reproduced. |
| `env -u FLEET_STATE fleet plan "add a --version flag to the cli"` | 3 | 55 | 264 | Partial `intent:`, `agent:`, `skills:` output precedes the environment reason. |
| `FLEET_STATE=var/fleet fleet ledger verify` | 0 | 27 | 0 | `verified checked=9 total=9`. |
| Same 9-row chain after changing `GENESIS` to `GENESIX` | 8 | 0 | 0 | Exact silent tamper defect reproduced. |
| `fleet sow --task "add a --version flag"` | 7 | 0 | 806 | Refusal names the missing citation and writes a corrected template. |
| Complete SOW, then `fleet sow accept --id <id>` | 9 then 0 | 1470/125 | 194/0 | SOW ready and accepted as expected. |
| Accepted SOW `fleet run ... --repo "$PWD" --agent stub` | 7 | 0 | 519 | End-to-end run was refused because this shared checkout is dirty. |
| Restored the copied chain, then `fleet ledger verify` | 0 | 27 | 0 | `verified checked=5 total=5`. |
| `bash tests/corpus/M9.sh` | 127 | 0 | 61 | `No such file or directory`; required detector is absent. |

The walkthrough's literal `fleet run --task ""` is therefore not the command
that produces its advertised result on this checkout. The silent result needs
the required `--repo` and `--agent` arguments, which the document does not
publish.

## Independent verifier

Command:

```text
FLEET_MUTANTS=0 bash verify.sh
```

Exit: `6`.

Real summary:

```text
  ok   detectors
  FAIL corpus                     (see var/verify.log)
-- 14 passed, 1 failed, 1 skipped (denominator: 16 stages) --
```

The failed corpus output includes:

```text
  TIMEOUT A1.sh exceeded 30s -- treated as FAILED
  TIMEOUT A3.sh exceeded 30s -- treated as FAILED
  TIMEOUT A4.sh exceeded 30s -- treated as FAILED
  TIMEOUT A8.sh exceeded 30s -- treated as FAILED
  TIMEOUT C11.sh exceeded 30s -- treated as FAILED
  TIMEOUT C2.sh exceeded 30s -- treated as FAILED
  TIMEOUT C22.sh exceeded 30s -- treated as FAILED
  TIMEOUT C26.sh exceeded 30s -- treated as FAILED
  TIMEOUT C6.sh exceeded 30s -- treated as FAILED
  TIMEOUT S6.sh exceeded 30s -- treated as FAILED
  TIMEOUT S9.sh exceeded 30s -- treated as FAILED
  TIMEOUT T1.sh exceeded 30s -- treated as FAILED
  TIMEOUT T15.sh exceeded 30s -- treated as FAILED
  TIMEOUT T5.sh exceeded 30s -- treated as FAILED
DENOMINATOR checked=34 total=34 excluded=69 caught=17
```

`bin/detector-integrity.sh` separately reports `105 detectors match the
manifest (denominator: 105)`, but that does not establish M9: `M9.sh` is not
one of those detectors and the corpus stage is red.

## Source-level confirmation

- `keel/fleet/src/main.rs:904-912` records an empty-task receipt and returns
  exit 7 without printing a reason.
- `keel/fleet/src/main.rs:2995-2999` prints only after `verify_rows` succeeds;
  mismatch returns at `3002-3035` have no diagnostic output.
- `keel/fleet/src/main.rs:2659-2664` prints the three plan fields before the
  state-dependent route call.

These are not stale prose-only findings; the debug binary rebuilt by the
verification run reproduced all three observable behaviors.

## Required changes before acceptance

1. Make the empty-task refusal print a reason naming the invalid input, while
   preserving exit 7 and the receipt invariant.
2. Make ledger mismatch verification print a useful reason and a published
   checked/total boundary before returning exit 8. Keep the success result
   honest and reject zero-row verification.
3. Validate `FLEET_STATE` before emitting plan fields, or buffer output until
   the environment check has passed. An environment refusal must not leave a
   partial plan on stdout.
4. Add `tests/corpus/M9.sh`. It must drive every refusable surface, assert at
   least one reason line for every non-zero exit, publish the candidate and
   checked denominator, and fail if it examines zero inputs. Add it to the
   manifest through the normal integrity workflow.
5. Mutation-test M9 and each new output guard: remove the guard, show the
   detector goes red with the relevant reason, restore it, and record the
   caught/total result. Do not accept a mutation run that measures zero cases.
6. Resolve the current corpus timeouts and rerun the exact verifier until its
   final exit is 0 with the final `passed/failed/skipped` denominator.
7. Update the walkthrough to publish the exact reproducing command lines,
   stream byte counts, ledger denominators, and a clean disposable target for
   the claimed successful end-to-end run. Do not state “the code is right” while
   these runtime defects remain.

