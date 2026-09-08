# Adversarial review: S2b-fix

Date: 2026-09-02  
Deliverable: `docs/delta.d/S2b-fix.md`  
Contract: `handover/BACKLOG.md:176-224`, especially acceptance at lines 222-224  
Verdict: **REJECT**

## What was claimed

The deliverable claims that:

1. `plan_command` degrades only `EXIT_ENV` router failures to a rendered plan and propagates every
   other typed error.
2. A fresh state with no `FLEET_METER_WINDOWS` prints the three-command plan and exits 0.
3. A corrupt persisted `meter-v1.tsv` propagates `EXIT_INVARIANT` and exits 6.
4. Two non-vacuous unit tests cover those paths and publish a denominator because the injected
   state closure is called exactly once.
5. `cargo test -p fleet --bin fleet plan_command` reports 2 passed, and
   `cargo test -p fleet --bin fleet` reports 103 passed / 0 failed / 1 ignored.

The S2b contract is larger than those claims. It requires a genuine code change to pass a normal
`git commit` without `--no-verify`, with the real verifier result pasted and the chosen resolution
path stated (`handover/BACKLOG.md:222-224`).

## What I actually ran

No Git command was run. The review instruction explicitly forbids Git because concurrent workers
share this checkout.

| Command / user path | Exit | Observed |
|---|---:|---|
| `cargo test -p fleet --bin fleet plan_command` from repo root | 101 | Cargo could not find `Cargo.toml`. |
| `cargo test -p fleet --bin fleet` from repo root | 101 | Same missing-manifest failure. |
| `cargo test --manifest-path keel/Cargo.toml -p fleet --bin fleet plan_command` | 0 | 2 passed, 0 failed, 0 ignored, 102 filtered out. |
| `cargo test --manifest-path keel/Cargo.toml -p fleet --bin fleet` | 0 | 103 passed, 0 failed, 1 ignored. |
| `cargo build --manifest-path keel/Cargo.toml -p fleet --bin fleet` | 0 | Checkout binary built successfully. |
| Fresh `mktemp -d` state, meter windows unset, real binary `fleet plan "add a --version flag"` | 0 | Printed routing-unavailable status and all 3 planned commands with denominator 3. |
| Fresh state containing corrupt `meter-v1.tsv`, same real binary command | 6 | Printed only the persisted-state invariant error; no command list was rendered. |
| Fresh state with `FLEET_METER_WINDOWS=''`, same real binary command | 0 | Explicit route-unavailable decision; command plan still rendered 3 of 3. |
| `bash tests/acceptance/swarm.sh` | 0 | `50 passed, 0 failed`; N9 reports 4 checked. |
| `FLEET_MUTANTS=0 bash verify.sh` | 6 | 17 passed, 1 failed, 1 skipped; corpus red. |

The fresh-state user output included:

```text
routed lane: UNAVAILABLE — routing could not be evaluated (see the environment fault printed above).
commands: 3 planned (denominator: 3)
  1. fleet sow
  2. fleet sow accept
  3. fleet swarm dispatch
FRESH_PLAN_EXIT=0
```

The corrupt-state user output was:

```text
fleet meter: invalid persisted planning state: unsupported meter state generation
CORRUPT_PLAN_EXIT=6
```

## Independent full verifier

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
  ok   recur
  ok   semgrep
  ok   trivy
  SKIPPED WITH A REASON mutants (set FLEET_MUTANTS=1 - full pass ~24min)
  ok   attest-smoke
  ok   pytest
  ok   detectors
  FAIL corpus (see var/verify.log)
-- 17 passed, 1 failed, 1 skipped (denominator: 19 stages) --
VERIFY_EXIT=6
```

The corpus terminal evidence from this run was:

```text
TIMEOUT-PERSISTENT M6.sh -- timed out again on an uncontended serial retry: a genuinely hanging detector. Counted as a caught regression.
DENOMINATOR checked=34 total=34 excluded=69 caught=7 timeout_contention=0 timeout_confirmed=0 timeout_persistent=1
```

## What I observed

### The narrow error-classification repair works

`keel/fleet/src/main.rs:2972-2982` now handles only `Err(EXIT_ENV)` benignly and returns every other
error code. Both corrected unit-test commands pass. More importantly, the built user binary renders
the fresh-state plan and returns 6 for the corrupt persisted meter. The original catch-all cheat is
not present in this checkout.

### The contract outcome is still absent

The current verifier exits 6. The unconditional hook runs that verifier, so it cannot presently
permit the contract's normal commit path. `handover/BACKLOG.md:191-194` says no commit landed and
keeps S2b `[~]`; `handover/PROGRESS.md` records `S2b-fix | in-progress` and no commit attempt. The
status records are now honest, but an honest incomplete result is still not acceptance.

### Both commands pasted by the deliverable are non-reproducible from the documented repo root

`docs/delta.d/S2b-fix.md:49-52` omits either `--manifest-path keel/Cargo.toml` or an explicit
`cd keel`. Both cited commands exit 101 exactly as written from the checkout root. The corrected
forms pass and confirm the claimed test counts, but the deliverable's commands do not reproduce its
own evidence.

### The claimed unit-test denominator is not published or asserted

The tests at `keel/fleet/src/main.rs:4487-4524` assert outputs and the propagated error. They do not
count closure invocations or emit/assert `{checked,total}` for the two route-error classes. `FnOnce`
limits a closure to at most one call; it does not itself publish evidence that it was called exactly
once. The behavioral paths are non-vacuous, but the deliverable's stronger denominator claim at
`docs/delta.d/S2b-fix.md:43-44` is unsupported as written.

### The arithmetic that is published

1. The user plan is correct: 3 numbered commands equal its published denominator of 3.
2. The focused test selects 2 S2b tests: 2 passed, with 102 other bin tests filtered out.
3. The full bin-test arithmetic is correct: 103 passed + 1 ignored = 104 total.
4. The verifier arithmetic is correct: 17 passed + 1 failed + 1 skipped = 19 stages.
5. Corpus accounts for every uppercase detector script: 34 checked + 69 excluded = 103; within
   checked, 27 passed + 7 caught = 34. Its one persistent timeout is explicitly included in caught.

The unresolved swarm counting defect from `docs/REVIEW-S2b.md` remains at
`tests/acceptance/swarm.sh:308-311`: four N9 prompts produce one aggregate pass when all succeed,
but each failed prompt produces its own failure. `4 checked` is now visible in the N9 label, but the
top-level assertion total is still outcome-dependent when multiple N9 prompts fail. The builder
must not edit `tests/acceptance/*`; this belongs to the lead acceptance-suite owner.

## Findings

### BLOCKER: S2b's acceptance criterion is not met

Failure -> no demonstrated normal commit succeeded, and the independent verifier exits 6.  
Cause -> corpus remains a real failing stage while the hook unconditionally executes the full
verifier.  
Fix -> resolve the corpus failures, or implement the contract's last-resort discriminating-hook
path with non-vacuous relevance evidence; then pass a normal commit containing a genuine code
change without `--no-verify`.

### P1: the deliverable's proof commands fail as written

Failure -> both cited commands exit 101 from the repo root.  
Cause -> the Cargo workspace manifest is under `keel/`.  
Fix -> use `cargo test --manifest-path keel/Cargo.toml ...` or explicitly document `cd keel`.

### P1: the denominator claim is stronger than the evidence

Failure -> the document says the state seam is called exactly once and calls that a published
denominator, but neither test counts calls or publishes checked/total.  
Cause -> `FnOnce` was treated as proof of exactly-once execution.  
Fix -> add an explicit call counter assertion and report a stable `{checked,total}` for the two
error classes, or remove the denominator claim and describe only the behavior actually asserted.

### P1: swarm's N9 top-level count remains outcome-dependent

Failure -> one all-green aggregate assertion can turn into up to four failure assertions.  
Cause -> the loop emits one `no` per failed prompt but emits one `ok` for all four successes.  
Fix -> the lead acceptance-suite owner must make the top-level assertion denominator stable while
retaining all four cases; the S2b builder must not edit `tests/acceptance/*`.

## Exact changes required before acceptance

1. Correct both verification commands so they run from the repository root and paste their complete
   results, including filtered/ignored counts.
2. Make the S2b test denominator real: assert an exact call count and publish a stable 2-of-2
   error-class total, or remove the unsupported denominator claim.
3. Have the acceptance-suite owner make N9's top-level count outcome-independent; do not let the
   S2b builder edit `tests/acceptance/*`.
4. Resolve every red verifier stage, or implement option (4) from the backlog with evidence that no
   relevant check was skipped and zero examined inputs cannot pass.
5. Run `FLEET_MUTANTS=0 bash verify.sh` again and paste the final denominator and exit code.
6. Make a genuine code change pass a normal `git commit` through the hook without `--no-verify`,
   paste the commit attempt, and state whether resolution used backlog path (2)+(3) or path (4).
   Only then may S2b move from `[~]` to `[x]`.
