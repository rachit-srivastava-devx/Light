# Adversarial review: S2b

Date: 2026-09-02  
Deliverable: `docs/delta.d/S2b.md`  
Contract: `handover/BACKLOG.md`, S2b acceptance at lines 190-192  
Verdict: **REJECT**

## Contract and claim

The contract requires all of the following:

1. A genuine code change is committed by a real `git commit`.
2. The pre-commit hook is not bypassed.
3. The commit succeeds.
4. The real `verify.sh` result is pasted.
5. The deliverable states whether the result came from fixing swarm/corpus or from making the hook discriminating.

The deliverable proves the narrower swarm repair, but it also states that `corpus` remains red, `verify.sh` remains red, and the real commit failed with `COMMIT_EXIT=1` (`docs/delta.d/S2b.md:176-217`). `handover/PROGRESS.md` nevertheless calls S2b `done`, and `handover/BACKLOG.md` marks it `[x]`. That is the known cheat "done whose log says blocked."

## What I actually ran

No Git command was run. The review instruction explicitly forbids Git because concurrent workers share this checkout. This means the cited `git apply -R`, `git diff`, `git commit`, `git log`, and `git status` commands were intentionally not repeated. The deliverable's own captured failed commit is already conclusive against its acceptance criterion.

### User-visible plan paths

Each command used the checkout binary, a fresh `mktemp -d` `FLEET_STATE`, `FLEET_SOW_BYPASS=1`, and no `FLEET_METER_WINDOWS`.

| Prompt | Exit | Observed |
|---|---:|---|
| `add a --version flag` | 0 | Change intent; routing unavailable; `commands: 3 planned (denominator: 3)`; includes `fleet sow`, `fleet sow accept`, `fleet swarm dispatch` |
| `what broke` | 0 | Diagnose intent; routing unavailable; `commands: 2 planned (denominator: 2)`; includes `fleet doctor`, `fleet ratchet show` |
| `handle division by zero` | 0 | Change plan rendered |
| `support UTF-8 filenames` | 0 | Change plan rendered |
| `validate the config on load` | 0 | Change plan rendered |
| `delete the dead retry path` | 0 | Change plan rendered |
| `make me a sandwich` | 7 | Refused with `no committed intent matches` and 3 near candidates |

The documented ordinary-input behavior is real.

### Acceptance suite

```text
$ bash tests/acceptance/swarm.sh
== 50 passed, 0 failed ==
SWARM_RUN_1_EXIT=0

$ bash tests/acceptance/swarm.sh
== 50 passed, 0 failed ==
SWARM_RUN_2_EXIT=0
```

Both current runs were green. This proves the narrow N* fix, not the S2b commit-unblocking contract.

### Rust tests

The cited command is not reproducible from the stated repository root:

```text
$ cargo test -p fleet
error: could not find `Cargo.toml` in `/Users/rachitsrivastava/youtube/Principal Engineering/fleet-rs` or any parent directory
CARGO_TEST_ROOT_EXIT=101
```

Running it from `keel/` resolves and is green:

```text
$ (cd keel && cargo test -p fleet)
lib:      8 passed, 0 failed
main:     95 passed, 0 failed, 1 ignored
trybuild: 1 passed, 0 failed
doc-test: 1 passed, 0 failed
CARGO_TEST_KEEL_EXIT=0
```

### Adversarial hard case: corrupt persisted meter

I wrote `not-a-valid-meter` to a fresh state's `meter-v1.tsv` and ran the same plan command:

```text
$ FLEET_STATE=<fresh-corrupt-state> fleet plan "add a --version flag"
fleet meter: invalid persisted planning state: unsupported meter state generation
routed lane: UNAVAILABLE — routing could not be evaluated (see the environment fault printed above).
commands: 3 planned (denominator: 3)
  1. fleet sow
  2. fleet sow accept
  3. fleet swarm dispatch
CORRUPT_METER_PLAN_EXIT=0
```

This is wrong. `meter::planning_snapshot_at` classifies corrupt persisted state as `EXIT_INVARIANT` (`keel/fleet/src/meter.rs:269-277`), but `plan_command` catches every `Err(_)`, mislabels it as an environment fault, and returns success (`keel/fleet/src/main.rs:2927-2953`). The repair weakened a typed invariant boundary to satisfy the fresh-state acceptance path.

### Independent full verifier

Attempt 1 reached corpus and was externally terminated with exit 143. It is not counted as evidence.

Attempt 2 completed:

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
VERIFY_ATTEMPT_2_EXIT=6
```

The corpus terminal line was:

```text
DENOMINATOR checked=34 total=34 excluded=69 caught=28 timeout_contention=0 timeout_confirmed=0 timeout_persistent=25
```

The real verifier remains red. Therefore the unconditional pre-commit hook remains a blocker and S2b's user outcome is not delivered.

## Arithmetic audit

1. Full verifier arithmetic is correct: `17 + 1 + 1 = 19` stages.
2. Corpus arithmetic is traceable: 34 checked + 69 excluded = 103 uppercase detector scripts currently present; 25 persistent timeouts are included in the 28 caught results. It is non-vacuous because `checked=34`.
3. Swarm's final summary does not publish a labeled denominator. More seriously, its denominator is outcome-dependent: the green run records `50 + 0 = 50`, while the documented red baseline records `45 + 8 = 53`.
4. The cause is `tests/acceptance/swarm.sh:308-311`: four N9 prompts collapse into one `ok` when all pass, but each failing prompt emits its own `FAIL`. The four cases are executed, but the top-level count changes with the verdict, so before/after totals cannot be compared as one stable denominator.

## Findings

### BLOCKER (confidence 10/10): S2b is marked done while its acceptance criterion failed

Failure -> real commit did not land.  
Cause -> the implementation redefined S2b as "make swarm green" even though the contract says "a real commit succeeds."  
Fix -> leave S2b incomplete until the full hook succeeds on a genuine code change without `--no-verify`.

### P1 (confidence 10/10): the fix swallows persisted-state invariant corruption

Failure -> corrupt `meter-v1.tsv` prints an invariant error but `fleet plan` exits 0 and calls it an environment fault.  
Cause -> `match route::for_plan(...) { ... Err(_) => continue }` discards the typed error.  
Fix -> continue only for `Err(EXIT_ENVIRONMENT)` if that behavior is intentional; propagate `EXIT_INVARIANT` and every other typed error. Add a unit test for corrupt persisted meter state. Do not edit `tests/acceptance/*`.

### P1 (confidence 10/10): swarm's test denominator changes with outcomes

Failure -> the cited red and green totals are 53 and 50, so the summary is not a stable measurement.  
Cause -> N9 emits one aggregate pass but up to four individual failures.  
Fix -> the lead-owned acceptance suite must record a fixed number of top-level assertions and print an explicit final denominator. The builder must not edit `tests/acceptance/*` under this repository's rules.

### P2 (confidence 10/10): one cited proof command does not run from the documented repo root

Failure -> `cargo test -p fleet` exits 101 at the repo root.  
Cause -> the workspace manifest is under `keel/`.  
Fix -> document `cargo test --manifest-path keel/Cargo.toml -p fleet` or explicitly show `cd keel`.

## Exact changes required before acceptance

1. Fix the catch-all routing error handling so persisted meter corruption exits 6 and add a non-acceptance unit test proving both the fresh-state exit-0 path and corrupt-state exit-6 path.
2. Have the acceptance-suite owner make N9's assertion count outcome-independent and publish the final denominator; builders must not modify the lead-owned suite.
3. Resolve the remaining corpus blocker, or use the contract's last-resort discriminating-hook option with documented audit evidence. Do not bypass or weaken the gate.
4. Run `FLEET_MUTANTS=0 bash verify.sh` to completion and require `0 failed` with its final denominator and exit 0.
5. Make a genuine code change and prove a normal `git commit` succeeds through `.githooks/pre-commit`, without `--no-verify`; paste the commit attempt and post-attempt proof. Only then may S2b be marked `[x]`/`done`.

