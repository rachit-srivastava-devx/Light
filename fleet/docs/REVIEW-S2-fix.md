# Adversarial review: S2-fix

**Deliverable:** `docs/delta.d/S2-fix.md`  
**Contract:** item `S2` in `handover/BACKLOG.md`, including the five rework demands recorded in the deliverable  
**Review date:** 2026-09-02  
**Verdict:** **REJECT**

The happy path is real: in one open tmux pane, an actual plain stub dispatch changed the TUI from
`LANES · 0 tracked` to `LANES · 1 tracked / Running`, then to `Passed`. That does not rescue the
deliverable. The contract file is invalid JSON and is never loaded by the validator; the live tail
can permanently skip a partially written receipt; the CLI's truly empty-input path returns the
wrong typed exit without a denominator; and the advertised root commands/evidence are not fresh.

## Contract disposition

| Requirement / reviewer demand | Result | Observed evidence |
|---|---:|---|
| Live console changes during an actual dispatch | PASS on the uncontended happy path | Same tmux pane: `0 tracked` -> `1 Running` -> `1 Passed`; dispatch exit `0` |
| Correct `lane-status.v1` contract exercised by a real dispatch | **FAIL** | The contract does not parse; the validator exits `0` without reading it |
| ADR before editing `contracts/*.json` | PASS | `docs/adr/ADR-0001-lane-status-projection.md` exists and records the projection decision |
| Non-vacuous regression with `{checked,total}` | **FAIL at the user-facing CLI boundary** | A genuinely empty ledger exits `6` silently before the row validator; no denominator is printed |
| Fresh, root-runnable evidence and correct totals | **FAIL** | Both cited root Cargo test commands exit `101`; corpus says `caught=7`, not `caught=8`; six named tests are listed as five |

**Demand count: checked=5, passed=2, failed=3.**

The backlog's separate scope statement is present in `docs/delta.d/S2.md`: the orb-side voice/graph
UI is explicitly identified as a different repository and out of scope.

## What was claimed versus observed

### 1. The declared contract is invalid and the validator does not exercise it

**Claim:** `contracts/lane-status.v1.json` is a real projection contract, and
`fleet contract lane-status validate` validates every projected row against it.

**[P1] (confidence: 10/10) Failure -> Cause -> Fix**

- **Failure:** the acceptance criterion "the contract file exists and is exercised by a real
  dispatch" is not met. `jq empty contracts/lane-status.v1.json` exits `5`; `python3 -m json.tool
  contracts/lane-status.v1.json` exits `1`. The literal newline inside the `comment` string at
  `contracts/lane-status.v1.json:5-7` is illegal JSON.
- **Cause:** `contract_lane_status_validate` at `keel/fleet/src/main.rs:3151` reads ledger rows and
  calls a handwritten subset validator. No production or test code loads
  `contracts/lane-status.v1.json`. The handwritten function also omits schema requirements such as
  `actor`, `ts_wall`, `agent`, `resolved_model`, date-time format, and `additionalProperties`.
- **Proof of bypass:** a real clean-fixture dispatch produced three lane rows; the command printed
  `lanes: valid (checked=3, total=3)` and exited `0` while both independent JSON parsers rejected
  the contract.
- **Fix:** make the contract valid JSON, load/compile that exact file, assemble one shared projection,
  and validate every real row with the schema. Add a regression that makes the schema unparsable or
  changes an enum and proves the CLI fails. A handwritten mirror is not proof that the contract was
  exercised.

### 2. The live tail permanently skips a receipt if polling lands between row and newline writes

**Claim:** `refresh_ledger` trims an incomplete trailing row and leaves those bytes for the next
poll; only complete appended rows are consumed.

**[P1] (confidence: 10/10) Failure -> Cause -> Fix**

- **Failure:** I replayed one real dispatch-emitted `lane_status` row through the actual TUI in two
  writes. After the first 216 of 432 bytes, the same open pane showed `LANES · 0 tracked`. After the
  remaining bytes plus newline were written, it still showed `LANES · 0 tracked`. The completed row
  was permanently skipped.
- **Cause:** `refresh_ledger` correctly slices through the last newline, but then advances
  `tail.offset` to `text.len()` at `keel/fleet/src/console.rs:784` and `:798`, including bytes after
  the last complete newline. On the next poll, parsing begins in the middle of the JSON row.
  This is reachable with the production writer because `append_receipt` writes the JSON and newline
  in two separate calls at `keel/fleet/src/main.rs:3444-3445`, while the console reader does not take
  the ledger lock.
- **Fix:** advance the consumed offset only to the byte after the last complete newline, or retain an
  explicit carry buffer. Add a regression that writes half a row, polls, writes the remainder and
  newline, polls again, and requires the lane to appear exactly once. Also cover same-size file
  replacement; the current "rewrite" test only replaces two rows with a shorter file.

### 3. The zero-input CLI path bypasses the advertised typed failure and denominator

**Claim:** `fleet contract lane-status validate` publishes `{checked,total}` and returns exit `8`
when `checked=0`.

**[P1] (confidence: 10/10) Failure -> Cause -> Fix**

- **Failure:** with a new empty `FLEET_STATE`, the command printed nothing and exited `6`. With one
  unrelated refusal receipt but zero lane rows, it correctly printed
  `checked=0, total=0, valid=0` and exited `8`.
- **Cause:** `contract_lane_status_validate` calls `ledger_rows(false)`. That helper returns exit `6`
  on an empty ledger at `keel/fleet/src/main.rs:3355-3356`, before
  `validate_lane_status_rows` can publish the denominator and return exit `8`. The unit test calls
  the pure inner function, so it misses the user-facing branch.
- **Fix:** read with `allow_empty=true`, always invoke the row validator, and add a CLI-level test
  for a missing/empty ledger that asserts output `checked=0,total=0` and exit `8`.

### 4. The reproduction and arithmetic are not fresh

**[P1] (confidence: 10/10) Failure -> Cause -> Fix**

- `cargo test -p fleet --bin fleet` from the repository root exits `101`: no root `Cargo.toml`.
- `cargo test -p fleet` from the repository root exits `101` for the same reason.
- `cargo fmt -p fleet` from the repository root exits `1` for the same reason.
- The deliverable's "exact reproduction" invokes `fleet` from `PATH`. On this machine that resolves
  to `/Users/rachitsrivastava/.local/bin/fleet`, an unrelated binary which exits `2` with
  `unknown verb swarm`. The checkout binary must be named explicitly.
- The corrected root commands do pass:
  - `cargo test --manifest-path keel/Cargo.toml -p fleet --bin fleet` -> exit `0`, `103 passed, 0
    failed, 1 ignored`.
  - `cargo test --manifest-path keel/Cargo.toml -p fleet` -> exit `0`, suites `8/0/0`,
    `103/0/1`, `1/0/0`, `1/0/0`.
  - `cargo fmt --manifest-path keel/Cargo.toml -p fleet -- --check` -> exit `0`.
- The six named tests in the deliverable are counted as five: four validator tests plus two refresh
  tests equals six.
- Fresh corpus output is
  `checked=34 total=34 excluded=69 caught=7 timeout_persistent=1`, not `caught=8` as claimed.
- **Fix:** replace every reproduction with an explicit checkout binary and root-runnable
  `--manifest-path keel/Cargo.toml` commands; rerun and paste current counts; publish the actual test
  denominator (`6` named tests).

### 5. Independent verifier result, including red

Command run from the repository root:

```text
FLEET_MUTANTS=0 bash verify.sh
```

Real final output and exit:

```text
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

The failing corpus ended with:

```text
M6: documented but missing: fleet arch
TIMEOUT-PERSISTENT M6.sh -- timed out again on an uncontended serial retry: a genuinely hanging detector. Counted as a caught regression.
DENOMINATOR checked=34 total=34 excluded=69 caught=7 timeout_contention=0 timeout_confirmed=0 timeout_persistent=1
```

The top-level `17 + 1 + 1 = 19` arithmetic is correct. The deliverable honestly reports the red
stage, but its internal corpus count is stale.

**[P1] (confidence: 10/10) Vacuous gate found:** the same verifier log contains
`recur-gate: checked=0 flagged=0`, yet the `recur` stage is marked `ok`. That violates the repository
rule that a gate examining zero inputs must fail. This is not the direct S2 implementation, but it
means the full verifier is currently granting a green stage without evidence. Fix the recur gate to
publish its input denominator and fail when `checked=0`.

Separate required acceptance command:

```text
bash tests/acceptance/p0.sh
== 34 passed, 0 failed ==
EXIT=0
```

## Commands actually run

| Command / user flow | Exit | Result |
|---|---:|---|
| `cargo test -p fleet --bin fleet` | 101 | No root `Cargo.toml` |
| `cargo test -p fleet` | 101 | No root `Cargo.toml` |
| `cargo fmt -p fleet` | 1 | No root `Cargo.toml` |
| Cited `fleet swarm dispatch ...` via `PATH` | 2 | Wrong binary; `unknown verb swarm` |
| Checkout debug binary against the shared dirty checkout | 7 | Honest `TARGET_REPO_NOT_CLEAN` refusal; zero lane rows |
| Checkout debug binary, plain stub dispatch, isolated clean fixture | 0 | `checked=5,total=5`; 3 lane rows: queued/running/passed |
| `fleet contract lane-status validate` on those 3 rows | 0 | `checked=3,total=3` despite invalid contract JSON |
| Same command with genuinely empty ledger | 6 | No output; wrong typed exit; no denominator |
| Same command with one non-lane receipt | 8 | `checked=0,total=0,valid=0` |
| Same open TUI plus actual dispatch | 0 | `0 tracked` -> `1 Running` -> `1 Passed` |
| Same open TUI plus split real row | 0 harness | Completed row remained unrendered (`0 tracked`) |
| `jq empty contracts/lane-status.v1.json` | 5 | Invalid JSON |
| `python3 -m json.tool contracts/lane-status.v1.json` | 1 | Invalid control character |
| Correct root bin test command | 0 | `103 passed, 0 failed, 1 ignored` |
| Correct root full package test command | 0 | All four suite totals pass |
| Correct root fmt check | 0 | Clean |
| `bash tests/acceptance/p0.sh` | 0 | `34 passed, 0 failed` |
| `FLEET_MUTANTS=0 bash verify.sh` | 6 | `17 passed, 1 failed, 1 skipped`; corpus red |

No `git` command was run. No acceptance test, deliverable, backlog, `DELTA.md`, contract, or
`keel/` file was edited.

## Exactly what must change before re-review

1. Repair `contracts/lane-status.v1.json` into valid JSON and make the production validator load
   and enforce that exact schema, including all required fields, formats, enums, and
   `additionalProperties`.
2. Fix `LedgerTail` so incomplete trailing bytes are retained across polls; add half-row and
   same-size-rewrite regressions outside `tests/acceptance/*`; rerun the same open-TUI proof.
3. Route missing/empty ledgers through `validate_lane_status_rows` so the CLI prints
   `checked=0,total=0` and exits `8`; add a CLI-boundary regression.
4. Replace the stale/unrunnable reproduction commands with explicit checkout-binary and
   `--manifest-path keel/Cargo.toml` commands; correct `5 -> 6` tests and `caught=8 -> caught=7`
   only after rerunning.
5. Make the `recur` verifier stage fail on `checked=0`, then rerun
   `FLEET_MUTANTS=0 bash verify.sh` and paste the complete final denominator including any red.

**Re-review gate:** all five changes above must be present. Do not mark S2 `[x]` merely because the
uncontended `0 -> Running -> Passed` demo works.
