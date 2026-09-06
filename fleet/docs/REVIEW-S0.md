# REVIEW — S0 kernel stabilization

Reviewer posture: adversarial. Contract: `handover/BACKLOG.md` item S0, especially lines 23–43.
Deliverable: `docs/delta.d/S0.md`. Review date: 2026-08-29.

## What was claimed

The deliverable claims that trybuild fixture drift is fixed, `recur` is wired and proven, the new
`fleet run --role` behavior has four unit tests, and the remaining corpus failure belongs to B13
rather than S0. It records a final result of `15 passed, 1 failed, 1 skipped (denominator: 17
stages)` and nevertheless marks S0 `[x]`.

The acceptance contract is narrower and non-negotiable: `FLEET_MUTANTS=0 bash verify.sh` must be
fully green with `0 FAIL`, the real output must be pasted into S0, `recur` must be included, the
quiet standalone corpus run must publish its real denominator, and S0 may be marked `[x]` only
then.

## What I actually ran

No standalone `git` command was run. Some required repo-owned scripts invoke `git` internally as
part of the behavior under review. The write-capable historical repair command `cargo fmt --all`
was not rerun because the review scope permits creating only this file; the exact non-mutating gate
form was run instead.

| Command / probe | Exit | Observation |
|---|---:|---|
| `cargo fmt --manifest-path keel/Cargo.toml --all -- --check` | 0 | Formatting is clean. |
| `CARGO_TARGET_DIR="$PWD/target-shared" cargo test --manifest-path keel/Cargo.toml -p fleet --test compile_fail illegal_lifecycle_transitions_do_not_compile -- --nocapture` | 0 | 1/1 trybuild test passed; all four compile-fail fixtures passed. |
| `CARGO_TARGET_DIR="$PWD/target-shared" cargo test --manifest-path keel/Cargo.toml -p fleet resolve_run_agent -- --nocapture` | 0 | 4/4 selected unit tests passed, but the builder-role case passed while routing refused and receipt writing faulted with `FLEET_STATE is not set`. |
| `bash bin/recur-gate.sh` | 0 | `checked=15 flagged=0 (signatures=1: E1)`. |
| `bash bin/recur-gate.sh --selftest` | 0 | 8/8 synthetic positive/negative checks passed. |
| Built CLI, isolated state, measured `codex=200000,claude=100000`, empty task, `--role builder` | 7 | Printed `routed: role=builder -> agent=codex (resolved model: codex; decided at stage 6)` before the intentional empty-task refusal. |
| Same CLI probe with `--agent stub --role builder` | 7 | Printed `routed: --role ignored, --agent was given explicitly`; explicit agent won. |
| Same CLI probe with `--role not-a-real-role` | 7 | Printed the unknown-role reason and full useful usage text. |
| `bash tests/acceptance/p0.sh` | 0 | `34 passed, 0 failed`. |
| `bash tests/acceptance/swarm.sh` standalone | 0 | `50 passed, 0 failed`; this does not erase the failure in the required full run below. |
| `bash tests/corpus/run.sh` standalone | 1 | 25 detectors timed out; `DENOMINATOR checked=34 total=34 excluded=69 caught=28`. |
| `FLEET_MUTANTS=0 bash verify.sh` | 6 | **16 passed, 2 failed, 1 skipped; denominator 19.** `swarm` and `corpus` failed. |

### Required independent verifier output, including red

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
  FAIL swarm                      (see var/verify.log)
  ok   policy
  ok   recur
  ok   semgrep
  ok   trivy
  SKIPPED WITH A REASON mutants   (set FLEET_MUTANTS=1 - full pass ~24min)
  ok   attest-smoke
  ok   pytest
  ok   detectors
  FAIL corpus                     (see var/verify.log)
-- 16 passed, 2 failed, 1 skipped (denominator: 19 stages) --
exit 6
```

The `swarm` log named the failure precisely:

```text
FAIL CC1 six concurrent dispatches on a cold store all succeed   1 of 6 failed
== 49 passed, 1 failed ==
```

The standalone swarm rerun passed 6/6, so this is intermittent or load-sensitive, not exonerated.
The required full verifier still observed a real failure.

The standalone corpus run reproduced the same 25 timeout IDs reported by S0 and exited 1. It also
emitted a concrete confounder absent from S0's causal claim:

```text
M2: 55494 files in the tree (>15000). Build artifacts are almost certainly inside the repo;
    set CARGO_TARGET_DIR outside it. Detectors walk the tree and will stall (D30).
DENOMINATOR checked=34 total=34 excluded=69 caught=28
```

Measured build-tree size was 3.3 GiB / 17,371 files in `target-shared` plus 3.8 GiB / 19,105 files
in `keel/target`. A quiet CPU/process list does not rule out filesystem tree-walk cost. Therefore
S0's attribution to process-spawn/subprocess overhead is a hypothesis, not a demonstrated cause.

## Findings

### 1. The item violates its explicit completion gate

`handover/BACKLOG.md:42-43` requires `0 FAIL` and says to mark `[x]` only then.
`docs/delta.d/S0.md:36-40` records one failed required stage and marks `[x]` anyway. Calling the red
stage “pre-existing” does not change the contract. Current verification is worse: two required
stages failed and the command exited 6.

### 2. The deliverable does not contain the required real output

`docs/delta.d/S0.md:36` labels a one-line summary “full paste.” It is not the stage-by-stage command
output and omits the failing corpus detail. This fails the literal acceptance requirement to paste
the real output including red.

### 3. The arithmetic is internally wrong and the corpus denominator is approximate

`docs/delta.d/S0.md:6-8` says `15 passed, 0 failed, 1 skipped ... of 17`; those categories add to
16, not 17. Lines 14–18 say `25 of ~105 detectors`, despite S0 explicitly requiring the real
denominator.

The live arithmetic is checkable only after reading implementation details:

- Manifest: 105 shell entries.
- `run.sh` and `_selftest.sh`: 2 support entries skipped before `total` is incremented.
- Runnable entries: 103 = 34 checked + 69 excluded.
- Checked outcomes: 34 = 28 caught + 6 clean.
- Caught outcomes: 28 = 25 timeouts + 3 non-timeout detector findings (`H1`, `M2`, `T2`).

The printed corpus denominator does not disclose the two support entries, so it does not reconcile
to the adjacent `105 detectors` statement without source inspection. S0's `~105` hides that gap.

### 4. The role unit tests are weaker than the S0 contract

The user-facing CLI behavior works under an explicitly routable meter fixture, but the tests do
not prove what S0 requires:

- `main.rs:3875-3905` accepts `codex`, `claude`, `freelane`, `EXIT_REFUSAL`, or `EXIT_ENV`. The test
  passed during this review while the router refused and receipt writing returned an environment
  fault. It does not prove “builder picks codex per `ORDER`.”
- The same test calls `resolve_run_agent(..., announce=false)`, so it cannot prove that the routed
  line is printed.
- `main.rs:3867-3872` checks only `EXIT_REFUSAL`; it does not capture or assert useful error text.

This is the known cheat class “green test, unproven feature,” even though the separately executed
CLI probes showed that the implementation can behave correctly with suitable state.

### 5. The S0 role path fabricates zero denominators on missing meter state

`route.rs:348-355`, reached by the new `--role` path, swallows every planning-snapshot error and
constructs `checked: 0, total: 0`, then invents a one-token requirement. Missing or invalid meter
state is absent evidence, not a measured zero. This violates the repo's hard `null is not 0` and
typed-error rules and lets a unit test normalize an environment fault as an acceptable outcome.

## Verdict

**REJECT**

Trybuild, `recur`, formatting, P0 acceptance, and the manually driven role behaviors are real. They
do not satisfy S0's completion contract because the required full verifier is red, the deliverable
knowingly marks `[x]` over that red result, the required role tests do not assert the promised
behavior, and the denominator evidence is neither exact nor internally consistent.

## Exactly what must change

1. Keep S0 open until `FLEET_MUTANTS=0 bash verify.sh` exits 0 with **0 failed required stages**;
   stabilize the intermittent cold-store `CC1` path and make corpus green under the documented
   in-repo build setup rather than waiving either failure as pre-existing.
2. Replace the permissive role tests with deterministic CLI-level assertions: seeded availability
   selects exactly `codex` and captures the routed line; explicit `--agent` wins; unknown role
   asserts exit 7 and useful text. `EXIT_ENV` must not count as success for the codex-selection case.
3. Stop converting a failed/missing planning snapshot to `{checked:0,total:0}`; propagate the typed
   error or represent unmeasured fields as absent.
4. Publish exact corpus arithmetic in S0: 105 manifest entries, 2 support scripts, 103 runnable,
   34 checked, 69 excluded, 28 caught, 6 clean, and the exact timeout count. Correct the
   `15+0+1` arithmetic.
5. Paste the actual stage-by-stage successful verifier output and its exit code into S0, then mark
   `[x]`; do not label a one-line summary a “full paste.”
