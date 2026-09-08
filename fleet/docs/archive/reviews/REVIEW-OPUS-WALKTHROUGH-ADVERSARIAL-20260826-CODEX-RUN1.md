# Adversarial review: opus walkthrough

Review ID: `OPUS-WALKTHROUGH-ADVERSARIAL-20260826-CODEX-RUN1`  
Reviewed: `docs/delta.d/opus-walkthrough.md` on 2026-08-26  
Verdict: **REJECT**

No direct Git command was run by the reviewer. I did not intentionally edit any repository file
other than this review; generated verifier output is reported as evidence below.

## Contract

There is no standalone `opus-walkthrough` item in `handover/BACKLOG.md`. The matching contract is
the unchecked B8 item, especially its three cited failure paths and its requirement that the
full verifier be green (`handover/BACKLOG.md:92-106`). The deliverable claims a complete user
walkthrough and three observed defects (`docs/delta.d/opus-walkthrough.md:3-22`).

## What was claimed

1. A full `plan` → refusal → run → SOW refusal → SOW acceptance → run → status → ledger verify →
   tamper → restore flow worked end to end.
2. `fleet run --task ""` exited 7 with zero bytes on both streams.
3. Tampered `fleet ledger verify` exited 8 with no output; the clean case was
   `verified checked=12 total=12`.
4. `fleet plan` with `FLEET_STATE` unset printed `intent:`, `agent:`, and `skills:` before an
   environment fault.

## Commands actually run

All probes used the checkout release binary at
`keel/target/release/fleet`, with temporary state directories created by `mktemp -d`.
Output byte counts are stdout/stderr separately.

| Probe | Exit | stdout / stderr | Observation |
|---|---:|---:|---|
| `FLEET_STATE=<tmp> fleet run --task ""` exactly as written | 7 | 0 / 187 | Does **not** reproduce the claimed silent case. It prints `fleet: run: --repo is required` and usage. |
| `FLEET_STATE=<tmp> fleet run --task "" --repo "$PWD" --agent stub` | 7 | 0 / 0 | Silent empty-task refusal reproduced. |
| `env -u FLEET_STATE fleet plan` | 7 | 0 / 41 | Usage refusal, not the claimed partial output. |
| `env -u FLEET_STATE fleet plan "add a --version flag"` | 3 | 55 / 264 | Three stdout lines (`intent`, `agent`, `skills`) precede the environment fault. |
| `FLEET_STATE=<tmp> fleet ledger verify` on a fresh state | 6 | 0 / 0 | Additional silent non-zero path omitted by the walkthrough. |
| 12 ledger appends, clean `fleet ledger verify` | 0 | 29 / 0 | `verified checked=12 total=12`. |
| Same chain after changing one hash byte, `fleet ledger verify` | 8 | 0 / 0 | Tamper claim reproduced exactly. |
| Restored saved chain, `fleet ledger verify` | 0 | 29 / 0 | `verified checked=12 total=12`. |

The SOW path was also exercised. A simple `fleet sow --task "add a --version flag"` exited 7
with 806 bytes explaining the missing challenge citation. A fully populated exact task exited 9
with a 1,502-byte SOW and a 194-byte acceptance prompt; `fleet sow accept --id <printed-id>`
exited 0 and printed `SOW_ACCEPTED`. Running that exact accepted task against the checkout then
exited 7 with 519 bytes because the target repo had uncommitted changes. A run without an
accepted SOW exited 7 with 290 actionable bytes. Thus the individual gates are observable, but the
document does not contain the task text, SOW id, repository, state path, binary identity, or
commands needed to reproduce this flow from the document alone.

## Arithmetic and honesty checks

- The 12/12 ledger denominator is arithmetically correct for the controlled 12-row fixture.
- The deliverable says “Two defects” but lists two defects and then a “third” defect. The count is
  three, not two.
- There is no denominator for the claimed “full flow”: no number of commands/cases attempted,
  no per-step result table, and no count of failure surfaces checked.
- The suggested “every non-zero exit” detector is broader than the three examples, but the
  walkthrough does not enumerate the surface it claims to cover. Fresh `ledger verify` already
  supplies a fourth silent non-zero case (`exit 6`).
- The current source explains the observed split: run validates required arguments before the
  empty-task check (`keel/fleet/src/main.rs:891-912`); plan prints its classification before
  calling `state_dir()` (`keel/fleet/src/main.rs:2659-2665`); ledger prints success only after
  verification succeeds (`keel/fleet/src/main.rs:2995-3000`).

## Mandatory independent verifier

Command run exactly:

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
  FAIL swarm (see var/verify.log)
  ok   policy
  SKIPPED WITH A REASON mutants (set FLEET_MUTANTS=1 - full pass ~24min)
  ok   attest-smoke
  ok   pytest
  ok   detectors
  FAIL corpus (see var/verify.log)
-- 13 passed, 2 failed, 1 skipped (denominator: 16 stages) --
exit=6
```

The red details in `var/verify.log` were:

```text
FAIL CC1 six concurrent dispatches on a cold store all succeed   1 of 6 failed
== 49 passed, 1 failed ==
detector-integrity: 105 detectors match the manifest (denominator: 105)
TIMEOUT A1.sh exceeded 30s -- treated as FAILED
TIMEOUT A3.sh exceeded 30s -- treated as FAILED
TIMEOUT A4.sh exceeded 30s -- treated as FAILED
TIMEOUT A8.sh exceeded 30s -- treated as FAILED
TIMEOUT B10.sh exceeded 30s -- treated as FAILED
TIMEOUT C1.sh exceeded 30s -- treated as FAILED
TIMEOUT C11.sh exceeded 30s -- treated as FAILED
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
DENOMINATOR checked=34 total=34 excluded=69 caught=25
```

## Findings

### F1 — The first central command is false as written

`docs/delta.d/opus-walkthrough.md:11` cites `fleet run --task ""` without the required
`--repo` and `--agent` arguments. Executed literally, it emits a useful missing-argument reason,
so the documented command does not produce the documented zero-byte result. The silent behavior
requires the hidden completion of the command shown in the table above.

**Required change:** publish the exact complete command and environment, or change the claim to
the literal command’s observed 187-byte error. Do not use a shorthand that changes which refusal
branch runs.

### F2 — The claimed end-to-end walkthrough is not replayable

`docs/delta.d/opus-walkthrough.md:3-6` provides a date and a sequence of nouns, not an executable
transcript. It omits the exact task, the first/refined SOW inputs, SOW id, target repo, state
directory, binary path/version, ledger fixture construction, tamper operation, restore operation,
stdout/stderr, and exit code for each step. A reviewer cannot run the claimed flow from this file.
The current checkout also cannot complete the accepted run against the shared dirty target, so
“works end to end” is not established by today’s independent run.

**Required change:** replace the sequence summary with a command/output/exit-code table that a
fresh operator can execute, using a clean temporary target repo and explicit state. Include the
exact tamper and restore commands and show the final verified denominator.

### F3 — Denominator and scope are missing

The only published denominator is the embedded 12/12 success example. It is not a denominator for
the full walkthrough or for the proposed detector. The document claims a broad “every non-zero
exit” property while naming only three cases and omitting a fourth silent case found on a fresh
ledger.

**Required change:** publish `checked`, `total`, and excluded/untriggerable counts for the
walkthrough’s tested surfaces, list every surface, and classify omitted cases as excluded rather
than silently treating the three examples as complete.

### F4 — The verifier is red, and mutants were not run

The B8 acceptance criterion requires `verify.sh` green. The required command exited 6 with two red
stages. `FLEET_MUTANTS=0` intentionally skipped mutation testing, so no mutation result exists for
the proposed detector in this review run.

**Required change:** fix or explicitly disposition the `swarm` and corpus failures, run the full
required gate to a final exit 0, then run the mutation gate for the new detector and publish its
caught/escaped/unviable denominator. A targeted green check is not a substitute.

## Required disposition for REJECT

1. Correct F1 and make the walkthrough replayable with exact commands, inputs, state, binary, and
   output evidence.
2. Add a denominator-backed table covering the claimed complete flow and the full refusal surface
   claimed by the detector; include fresh empty-ledger verification.
3. Resolve the red verifier stages and run the required mutation test. Re-review only after the
   final `verify.sh` result is green and pasted with its exit code.
