# Adversarial review: opus walkthrough

Date: 2026-08-26  
Deliverable: `docs/delta.d/opus-walkthrough.md`  
Reviewer: adversarial runtime review  

## Contract and verdict

The requested `opus-walkthrough` item does not exist in `handover/BACKLOG.md`. The
current backlog reference is B8 at `handover/BACKLOG.md:92-106`, which uses this
document as evidence and requires: all three reported paths fixed, `tests/corpus/M9.sh`
with a published denominator and mutation test, and a green `verify.sh`.

Verdict: **REJECT**.

The document correctly identifies two silent failure paths and partial output from
`plan`, but it does not provide reproducible commands or a denominator for the claimed
full flow. The required gate is also not met: M9 is missing and the independent verifier
exited 6 with a failed corpus stage.

## What the deliverable claims

- A complete user flow works: `plan` → plan refusal → run without SOW → SOW refusal →
  SOW acceptance → run → status → ledger verify → tamper → restore.
- Empty-task `run` exits 7 with zero bytes on both streams.
- Tampered `ledger verify` exits 8 with no output.
- An unset-state `plan` prints `intent:`, `agent:`, and `skills:` before an environment
  fault.
- A detector should require at least one reason line for every non-zero exit.

## Commands actually run

Binary used: `keel/target/debug/fleet` (the `fleet` command itself was not on `PATH`).
All state was isolated under fresh `mktemp -d` directories. No Git command was run.

| Case | Exit | stdout bytes | stderr bytes | Observation |
|---|---:|---:|---:|---|
| `FLEET_STATE=<fresh> fleet run --task ""` | 7 | 0 | 187 | Refused earlier because `--repo` is required; it printed the usage/reason. |
| `FLEET_STATE=<fresh> fleet run --task "" --repo "$PWD" --agent stub` | 7 | 0 | 0 | The reachable empty-task branch is silently reproduced. |
| `env -u FLEET_STATE fleet plan` | 7 | 0 | 41 | Only the usage line; this is not the document's partial-output case. |
| `env -u FLEET_STATE fleet plan "add a --version flag to the cli"` | 3 | 55 | 264 | Prints `intent:`, `agent:`, `skills:` then reports `FLEET_STATE is not set`. |
| clean ledger `fleet ledger verify` after one append | 0 | 27 | 0 | `verified checked=1 total=1`. |
| tampered ledger `fleet ledger verify` | 8 | 0 | 0 | Silent hash-chain mismatch reproduced. |
| restored ledger `fleet ledger verify` | 0 | 27 | 0 | `verified checked=1 total=1`. |

The literal empty-task command in the document is underspecified: it does not reach the
empty-task branch unless the required `--repo` and `--agent` arguments are added. The
corrected invocation does reproduce the defect.

## Full-flow attempt

I drove the flow with a disposable state directory and a task containing valid SOW
sections and a real citation (`keel/fleet/src/main.rs:40`). Results:

| Step | Exit | Result |
|---|---:|---|
| valid `fleet plan` | 0 | Printed 3 planned commands and `denominator: 3`; lane unavailable was reported. |
| `fleet plan "make me a sandwich"` | 7 | Refused with 3 candidates. |
| valid `fleet run` before SOW acceptance | 7 | Printed SOW id and recovery commands. |
| first `fleet sow --task ...` | 7 | Printed an 806-byte corrected template for the missing citation. |
| valid `fleet sow --task ...` | 9 | Printed a 1,439-byte SOW and `SOW_READY_AWAITING_REVIEW`. |
| `fleet sow accept --id <id>` | 0 | Printed `SOW_ACCEPTED`. |
| accepted `fleet run` with the exact accepted task | 7 | Refused because the checkout had uncommitted changes; no artifact was produced. |
| `fleet status` | 0 | Reported `1 of 1` task, failed with `SOW_NOT_ACCEPTED` from the earlier exact-task mismatch. |
| `fleet ledger verify` | 0 | `verified checked=3 total=3`. |

Therefore the claimed end-to-end successful run was not reproduced. The document gives
no exact task text, SOW payload, repository fixture, state directory, or acceptance id
with which a clean successful run can be independently repeated.

## Arithmetic and known-cheat checks

- The document publishes no denominator for its “full flow,” no count of tested failure
  surfaces, and no count of classified versus omitted non-zero exits.
- The only published historical count is `checked=12 total=12` on the ledger success
  path. It has no command, fixture, or ledger contents in the document, so it is not
  independently checkable.
- `tests/corpus/M9.sh` is missing. The corpus directory currently contains 103 detector
  scripts after excluding `run.sh` and `_selftest.sh`; the proposed M9 denominator is
  therefore not present or published.
- A fresh state independently returns `status --json` with `exit 0` and:

  ```json
  { "checked": 0, "total": 0, "empty": true, "groups": [...] }
  ```

  This is a vacuous success under the repo's own zero-input rule. Fresh `ledger verify`
  separately returns `exit 6` with 0 stdout bytes and 0 stderr bytes, another silent
  non-zero path omitted from the document's list.

## Mandatory verifier result

Command:

```text
$ FLEET_MUTANTS=0 bash verify.sh
```

Observed output, including the red result:

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
  SKIPPED WITH A REASON mutants (set FLEET_MUTANTS=1 - full pass ~24min)
  ok   attest-smoke
  ok   pytest
  ok   detectors
  FAIL corpus (see var/verify.log)
-- 14 passed, 1 failed, 1 skipped (denominator: 16 stages) --
```

`var/verify.log` records these corpus timeout failures, each treated as failed:
`A3.sh`, `A4.sh`, `A8.sh`, `C11.sh`, `C2.sh`, `C22.sh`, `C26.sh`, `C6.sh`, `S6.sh`,
`S9.sh`, `T1.sh`, `T15.sh`, and `T5.sh`. It also reports `M2: 34395 files (>15000)`
and `M6: 22 of 23 documented commands exist`. Its final detector arithmetic is:

```text
DENOMINATOR checked=34 total=34 excluded=69 caught=17
```

That is a failed verifier result, not green evidence for B8.

## Required changes before acceptance

1. Restore an explicit `opus-walkthrough` backlog item, or explicitly declare B8 as the
   contract for this deliverable. The current request names a contract that is absent.
2. Rewrite the walkthrough with exact, runnable commands: binary path, fresh state path,
   clean disposable repository setup, exact task/SOW text, acceptance id handling, and
   the tamper/restore file operation. Publish every case as `N of M`, including omitted
   refusal surfaces and the zero-input cases.
3. Fix the three documented user-facing defects: empty-task refusal must print a reason,
   tampered ledger verification must print a reason, and `plan` must validate required
   environment before emitting plan output.
4. Add `tests/corpus/M9.sh` to exercise every in-scope refusable surface, fail on zero
   output, publish its denominator, and mutation-test both the detector and a genuinely
   silent failure.
5. Resolve the corpus timeout failure and rerun exactly `FLEET_MUTANTS=0 bash verify.sh`;
   acceptance requires the final exit code and summary to be green.

