# Adversarial review: opus walkthrough

Date: 2026-08-25  
Deliverable: `docs/delta.d/opus-walkthrough.md`  
No git commands were run. The only repository file created by this review is this report.

## Verdict

**REJECT**

The two core silent paths are real, and the accepted-SOW happy path works on a clean disposable
target. The deliverable is still not acceptance evidence: its exact empty-task command is stale,
the full flow has no reproducible setup or denominator, the proposed M9 detector is absent, and
`FLEET_MUTANTS=0 bash verify.sh` exited 6.

## Contract

There is no standalone `opus-walkthrough` item in `handover/BACKLOG.md`. The only reference is B8
at lines 94-96. Treating B8 as the governing contract, its acceptance criteria at lines 101-106
require all three paths fixed, `tests/corpus/M9.sh` with a published denominator, mutation
evidence, and a green verifier. B8 remains `[ ]`.

## What was claimed

- Lines 3-6 claim a complete plan/refusal/SOW/accepted-run/status/ledger/tamper/restore flow works.
- Lines 8-18 claim three failure cases: silent empty-task run, silent tampered-ledger verify, and
  partial output from `plan` with `FLEET_STATE` unset.
- Lines 8-9 say there are “Two defects,” then line 16 adds “A third.”
- Lines 20-22 propose a detector requiring a reason on every non-zero exit.

## What I actually ran

All runs used the existing release binary at `keel/target/release/fleet`. Byte counts are stdout /
stderr, and each exit code was captured immediately.

| Case | Exit | Bytes | Observation |
|---|---:|---:|---|
| `fleet plan "add a --version flag to the cli"` | 0 | 499 / 0 | Plan rendered; `commands: 3 planned (denominator: 3)`. |
| `fleet plan "make me a sandwich"` | 7 | 0 / 154 | Refused with three candidates. |
| `fleet run --task "add a --version flag" --repo "$PWD" --agent stub` before SOW | 7 | 0 / 290 | Refusal named the SOW id and next commands. |
| `fleet sow --task "add a --version flag"` | 7 | 0 / 806 | Refusal named the missing citation and printed a template. |
| Complete multiline SOW from `README.md` | 9 | 1408 / 194 | `SOW_READY_AWAITING_REVIEW`; id emitted. |
| `fleet sow accept --id <id>` | 0 | 125 / 0 | `SOW_ACCEPTED`. |
| Accepted run against this checkout | 7 | 0 / 519 | Correctly refused the dirty target repository. |
| Accepted run against clean `/private/tmp/userdrive/myrepo` | 0 | 421 / 0 | Artifact `4d12039793961ee16cd528685d7ddf78532815b5accc2c5fbcffa6b7449e6668`; both oracles accepted it. |
| `fleet status` after that run | 0 | 2565 / 0 | `1 of 3` DONE; two earlier refusal receipts are also included. |
| `fleet ledger verify` before tamper | 0 | 29 / 0 | `verified checked=15 total=15`. |
| `fleet attest verify 4d12039793961ee16cd528685d7ddf78532815b5accc2c5fbcffa6b7449e6668` | 0 | 83 / 0 | `verified artifact=...`. |
| Same ledger after changing a stored hash | 8 | 0 / 0 | Silent mismatch reproduced. |
| Restored ledger verification | 0 | 29 / 0 | `verified checked=15 total=15`. |
| Exact cited `fleet run --task ""` | 7 | 0 / 187 | Does not reproduce the claim; it first reports `--repo is required`. |
| `fleet run --task "" --repo "$PWD" --agent stub` | 7 | 0 / 0 | Underlying silent empty-task refusal reproduced. |
| `env -u FLEET_STATE fleet plan "add a --version flag to the cli"` | 3 | 55 / 264 | `intent:`, `agent:`, `skills:` printed before environment fault. |
| `env -u FLEET_STATE fleet plan` | 7 | 0 / 41 | Usage only; no partial plan. |
| Fresh `fleet status --json` | 0 | 452 / 0 | `checked: 0`, `total: 0`, `empty: true`: vacuous success. |
| Fresh `fleet ledger verify` | 6 | 0 / 0 | An additional silent non-zero path omitted by the walkthrough. |

The successful run proves the happy path is possible, but not the document's unqualified
“end-to-end” claim as written: the required clean-target setup, exact task, artifact id, status
denominator, and tamper operation are absent. The `12/12` example is not reproducible from the
document; this replay produced `15/15` because the state already contained receipts.

## Arithmetic and known-cheat checks

- The prose counts two defects and then records a third: the stated count is inconsistent.
- There is no denominator for the walkthrough itself: no `checked/total` command or surface count,
  no success/refusal total, and no classification of omitted or untriggerable cases. `12/12` is
  only a ledger-row denominator, not coverage of the flow.
- `tests/corpus` contains 105 shell detectors and 105 nonblank manifest entries; `tests/corpus/M9.sh`
  does not exist. The only non-review M9 references are the B8 requirements themselves. No M9
  mutation result exists.
- The current acceptance script does not merely assert exit codes: its D21 block checks that a
  missing-`FLEET_STATE` run names `FLEET_STATE` and has at least 60 output bytes
  (`tests/acceptance/p0.sh:159-166`). The walkthrough's blanket explanation that the suite only
  checks exit codes is therefore inaccurate, even though the exact three paths are not covered.
- A fresh status check returns `0/0` with exit 0, and a fresh ledger verify returns exit 6 with no
  output. Both are hard cases relevant to an “every non-zero exit” detector and are silently absent.
- The implementation corroborates the observed defects: empty-task handling records a refusal and
  returns without printing (`keel/fleet/src/main.rs:904-912`), while ledger verification prints only
  after `verify_rows` succeeds (`keel/fleet/src/main.rs:2995-3000`).

## Required independent verifier

Command run exactly:

```text
$ FLEET_MUTANTS=0 bash verify.sh
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

Exit code: **6**.

`var/verify.log` records these corpus failures: `A4`, `C11`, `C2`, `C22`, `C6`, `S6`, `S9`, `T1`,
and `T15` timed out after 30 seconds and were treated as failed. It also reports `M2: 32074 files`
and the corpus summary `DENOMINATOR checked=34 total=34 excluded=69 caught=12`. This is red evidence,
not a green gate. Mutants were explicitly skipped by the required `FLEET_MUTANTS=0` invocation.

## Exact changes required for acceptance

1. Restore a real `opus-walkthrough` backlog item, or explicitly rename this deliverable as B8
   evidence and state that contract.
2. Correct the empty-task transcript to include `--repo` and `--agent`, and report both the stale
   shorthand behavior and the valid silent refusal. Fix the refusal to emit a reason.
3. Make tampered-ledger verification emit a reason while preserving the mismatch exit code. Decide
   and document the empty-store `status --json` and empty-ledger behavior instead of omitting them.
4. Reproduce the accepted run on a disposable clean target with exact setup, artifact, status,
   attestation, ledger, tamper, restore commands, exit codes, and a walkthrough `checked/total`
   denominator. Correct “Two defects” to three.
5. Implement `tests/corpus/M9.sh` across every refusable surface, fail on zero checked inputs,
   publish its denominator, mutation-test both reason and no-reason directions, and rerun the
   verifier until its final exit code is 0 with the full 16-stage denominator.
