# Adversarial review: opus walkthrough

Date: 2026-08-25

Verdict: **REJECT**

## Contract used

There is no standalone `opus-walkthrough` item in `handover/BACKLOG.md`. The only matching
reference is B8 at lines 92-106, so I used B8 as the contract. B8 requires:

- the three reported failure-path defects to be fixed;
- `tests/corpus/M9.sh` to exist, enumerate refusable surfaces, and publish its denominator;
- M9 to be mutation-tested; and
- `verify.sh` to be green.

The deliverable does not claim that these B8 requirements are already implemented. It claims the
walkthrough works end to end and reports the defects found by driving it.

## What the deliverable claims

`docs/delta.d/opus-walkthrough.md` claims that:

1. A `plan` -> SOW refusal -> SOW acceptance -> `run` -> `status` -> ledger verify -> tamper ->
   restore flow works end to end.
2. `fleet run --task ""` exits 7 with zero bytes on both streams.
3. A tampered `fleet ledger verify` exits 8 with no output; the clean path prints
   `verified checked=12 total=12`.
4. `fleet plan` with `FLEET_STATE` unset prints partial plan output before exit 3.
5. A future detector should require a reason on every non-zero exit.

## What I ran

I did not run git. I built an isolated review binary with:

```text
CARGO_TARGET_DIR=/tmp/fleet-rs-review-target cargo build --manifest-path 'keel/fleet/Cargo.toml' --bin fleet
BUILD_EXIT=0
```

All CLI checks below used temporary `FLEET_STATE` directories. Stream byte counts are stdout /
stderr.

| Invocation | Exit | Output bytes | Observation |
|---|---:|---:|---|
| `fleet run --task ""` exactly as written | 7 | 0 / 187 | Does not reproduce the claimed silent path. It visibly says `--repo is required` and prints usage. |
| `fleet run --task "" --repo "$PWD" --agent stub` | 7 | 0 / 0 | Reproduces the valid empty-task silent refusal. |
| `env -u FLEET_STATE HOME=<temporary> fleet plan "diagnose fleet"` | 3 | 61 / 264 | Reproduces partial `intent`, `agent`, `skills` stdout before a stderr environment-fault explanation. |
| `fleet plan "make me a sandwich"` | 7 | 0 / 154 | Refusal is actionable and names three candidates. |
| `fleet run --task "add a --version flag" --repo "$PWD" --agent stub` before SOW | 7 | 0 / 290 | Refusal names `SOW_NOT_ACCEPTED` and the accept command. |
| `fleet sow --task "add a --version flag"` | 7 | 0 / 806 | Refusal names the missing evidence citation and prints a corrected template. |
| Filled SOW | 9 | 1502 / 194 | Emits `SOW_READY_AWAITING_REVIEW` and an acceptance command. |
| `fleet sow accept --id <generated-id>` | 0 | 125 / 0 | Human acceptance succeeds. |
| Accepted `run` against this checkout | 7 | 0 / 519 | Refuses because the shared checkout has uncommitted changes; the successful end-to-end run was not independently reproduced. |
| `fleet status` after that refusal | 0 | 2571 / 0 | Renders a receipt-based status report with a non-zero task denominator. |
| `fleet ledger verify` on a fresh empty state | 6 | 0 / 0 | Additional silent invariant failure omitted by the deliverable. |

The SOW acceptance sequence itself is real. The claimed successful post-acceptance run is not
replayable in this shared checkout because the product correctly refuses the dirty target. The
document supplies no clean-repository setup or transcript that makes its end-to-end claim
independently replayable.

## Ledger arithmetic and tamper replay

In a separate fresh temporary state I performed exactly 12 successful ledger appends:

```text
append_01_exit=0
append_02_exit=0
append_03_exit=0
append_04_exit=0
append_05_exit=0
append_06_exit=0
append_07_exit=0
append_08_exit=0
append_09_exit=0
append_10_exit=0
append_11_exit=0
append_12_exit=0
GOOD_VERIFY exit=0 stdout_bytes=29 stderr_bytes=0
verified checked=12 total=12
TAMPERED_VERIFY exit=8 stdout_bytes=0 stderr_bytes=0
RESTORED_VERIFY exit=0 stdout_bytes=29 stderr_bytes=0
verified checked=12 total=12
APPEND_FAILURES=0
```

This isolated 12/12 arithmetic is correct. It does not establish a denominator for the complete
walkthrough or for all refusable surfaces. The document lists three defects but publishes no
`checked/total` population, no exclusions, and no classification of the other CLI failure paths.

## Independent verifier

Command executed exactly as required:

```text
FLEET_MUTANTS=0 bash verify.sh
```

Real result:

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
VERIFY_EXIT=6
```

The failing corpus evidence includes:

```text
  TIMEOUT A1.sh exceeded 30s -- treated as FAILED
  TIMEOUT C1.sh exceeded 30s -- treated as FAILED
  B8 skill injection verification failed and copied a symlink
  TIMEOUT T15.sh exceeded 30s -- treated as FAILED
  TIMEOUT T20.sh exceeded 30s -- treated as FAILED
DENOMINATOR checked=34 total=34 excluded=69 caught=24
```

This is not a green acceptance gate. Mutation testing was not run by this command, and no M9
result exists.

## Findings

1. **REJECT: the main empty-task command is inaccurate as written.** The exact command in the
   deliverable is visibly rejected earlier for missing `--repo` (exit 7, 187 stderr bytes). The
   silent defect requires the complete invocation with `--repo` and `--agent`. The evidence must
   show the exact runnable command, not a shorthand that reaches a different validation branch.

2. **REJECT: a valid empty-task refusal is still silent.** The complete invocation exits 7 with
   0 stdout and 0 stderr. This is the central user-visible defect the B8 contract says must be
   fixed.

3. **REJECT: tampered and empty-ledger failures are still silent.** Tampering exits 8 with 0/0,
   and an actually empty ledger exits 6 with 0/0. The deliverable reports only the tampered case
   and does not classify the empty-ledger surface.

4. **REJECT: the partial-plan behavior remains.** The missing-state invocation exits 3 after
   emitting three plan lines. It does include a stderr reason, so it is not a zero-message
   refusal; it is still the partial-output defect described by the deliverable and B8's “fix the
   three” requirement.

5. **REJECT: the required detector is absent.** `tests/corpus/M9.sh` does not exist. There is no
   published checked/total result for all refusable surfaces and no evidence that an
   untriggerable surface fails the detector rather than being dropped.

6. **REJECT: mutation evidence is absent and the mandatory verifier is red.** The recorded
   `verify.sh` result is exit 6 with one failed stage. B8 acceptance therefore cannot be claimed,
   regardless of the green unit and acceptance sub-stages.

## Exactly what must change

1. Fix the valid empty-task refusal and tampered-ledger mismatch so every non-zero path emits a
   human-readable line naming the reason. Fix the partial-plan path so it does not emit a plan
   before the environment check fails.
2. Add `tests/corpus/M9.sh`. It must execute every refusable CLI surface, assert a non-zero exit
   has a reason line, publish `checked`, `total`, and exclusions, and fail when a surface cannot be
   triggered or when `checked=0`.
3. Mutation-test M9 in both directions: remove or bypass its reason assertion and show M9 red;
   restore it and show green. Record the actual result.
4. Resolve the current red corpus stage and rerun `FLEET_MUTANTS=0 bash verify.sh` to exit 0 with
   the final `14/1/1` failure gone and the corpus denominator visible.
5. Rewrite the walkthrough with exact invocations, isolated state/repository preconditions, a
   denominator for every tested surface, and a clear distinction between a successful run and a
   dirty-repository refusal.
