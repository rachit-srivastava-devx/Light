# S2b fix — adversarial rework: `plan_command` no longer swallows router faults

Follow-up to `docs/delta.d/S2b.md`, driven by `docs/REVIEW-S2b.md` (verdict: REJECT).
The original S2b fix (rolling the meter abort) was about that abort swallowing the plan output.
The adversarial reviewer found a **second, deeper` fault**: `plan_command` caught the router's
error with a **catch-all** `Err(_)`, so it could not distinguish "no meter windows" (benign) from
an **invariant-class router fault** (a blinking red light that must propagate, exit 6). A corrupt
meter file or any other `EXIT_INVARIANT`-class fault would silently print the command list and
exit 0 — a gate that read zero inputs and *passed*, exactly the failure `AGENTS.md` rule 6 exists
to stop. `docs/REVIEW-S2b.md` (see `## Verdict`) records this.

## Change

`keel/fleet/src/main.rs`:

- `plan_command` refactored into `plan_command(args)` + `plan_command_lines(args, state: impl
  FnOnce() -> Result<PathBuf, i32>)` — the injectable state seam makes both paths unit-testable
  without a real `FLEET_STATE`.
- The router match is narrowed from `Err(_) => <benign fall-through>` to
  `Err(EXIT_ENV) => <benign fall-through, print command list, exit 0>` and
  `Err(code) => return Err(code)`.
- Comment updated: only an *environment-class* fault (missing tool / no meter windows, the
  `3` code) is benign for `plan`; every other typed error — notably `EXIT_INVARIANT` `6`,
  `EXIT_REFUSAL` `7`, `EXIT_MISMATCH` `8` — propagates to the caller instead of being
  swallowed.

## Before / after behavior

| Input | Before | After |
|---|---|---|
| Fresh state, `FLEET_METER_WINDOWS` unset (env fault, `3`) | prints meter message + command list, exit 0 | **same (correct, unchanged)** |
| Corrupt `meter-v1.tsv` (invariant fault, `6`) | prints command list, **exit 0** — fault swallowed | propagates `Err(6)`, exit **6** |

The first row was already handled by S2b's original fix; the second row is what this rework
closes.

## Unit tests (both paths, non-vacuous, in `main.rs`)

- `plan_command_rate_limits_router_faults_to_environment_class_only` — fresh state → router
  returns env-class `EXIT_ENV` → `Ok`, command-list text, exit 0.
- `plan_command_propagates_a_corrupt_meter_invariant` — injected state whose meter snapshot
  yields `EXIT_INVARIANT` → the error propagates as `Err(6)`, not a benign exit-0.
- **Publishes the denominator**: the injected-state closure is called exactly once and returns a
  `Result`, so a zero-examined path inverts the test outcome.

## Verification

```
$ cargo test -p fleet --bin fleet plan_command
test result: ok. 2 passed; 0 failed; 0 ignored   # both S2b paths
$ cargo test -p fleet --bin fleet
test result: ok. 103 passed; 0 failed; 1 ignored
```

`keel/fleet/src/route.rs`, `crew/crew/adapters/`, `tests/acceptance/*` untouched.