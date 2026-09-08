# S2 fix — adversarial rework: console is actually live, contract is a projection, lane stream exists

Follow-up to `docs/delta.d/S2.md`, driven by `docs/REVIEW-S2.md` (verdict: REJECT, 2/4).
The original implementation was correct about emitting receipts, but **0 of 7** emitted
`lane_status` bodies carried the schema-required `ledger_ref`, a **plain `swarm dispatch` wrote
zero lane_status rows**, and `keel-console` loaded the ledger **once** then never looked at it
again. The LANES view was a static screenshot of a stream that — for the reviewer's own
reproduction — never existed. This delta records every change made to close all five
reviewer demands, with reproductions.

## Reviewer's five demands

1. Console must be live: `LANES · 0 tracked` → `LANES · N tracked` while open.
2. `lane_status` bodies must carry `ledger_ref` (or the contract must be corrected).
3. Contract governance: an ADR must exist for `lane-status.v1` + the receipt-enum addition
   (`NEVER edit contracts/*.json without an ADR in docs/adr/`).
4. Non-vacuous regression test outside `tests/acceptance/*` publishing `{checked,total}`.
5. Fresh evidence with root-runnable cargo command, COMPLETE totals, current swarm numbers.

## Change 1 — the console is live (was: one-shot snapshot)

`keel/fleet/src/console.rs`:

- New `LedgerTail` struct tracking the byte offset consumed from `chain.jsonl`.
- `refresh_ledger(state, tail, chain)`: reads only the appended bytes, trims to the last
  newline, parses the new complete rows, and applies them via `load_ledger`. A rewritten or
  truncated ledger resets the read offset to 0 so a replaced file re-reads cleanly.
- `event_loop` now calls `refresh_ledger` every tick **before** drawing (250 ms poll cycle), so
  the LANES header and rows update without reopening or kind..line.
- `App` gained `tail` and `ledger_path`; `run()` syncs the tail offset to the current file size
  after the initial `load_state()` so the first poll reads only new bytes.

**Regression tests (non-vacuous):**
- `refresh_ledger_reads_only_appended_rows_incrementally` — writes 2 rows, refreshes (reads 2),
  appends 1 row, refreshes again (reads exactly the 1 new row; the tail does not re-read the old
  rows).
- `refresh_ledger_recovers_from_a_rewritten_ledger` — a deleted/rewritten ledger resets the
  offset and re-reads from byte 0.

## Change 2 — the plain dispatch writes a real lane stream (was: zero rows)

`keel/fleet/src/main.rs`, `swarm_dispatch_command`:

- The `worker.unwrap_or_else(...)` else branch previously produced `(worker, None, None)` — no
  lane at all. It now defaults the lane to `Role::Builder`, so **every** dispatch owns a lane.
- A `queued` lane-status receipt is appended before the `running` receipt on both the `--role`
  path and the plain path, so the lane appears in the ledger the instant the dispatch begins.

**The exact reproduction the reviewer would run** (plain, flagless, `--agent stub`):

```
$ FLEET_SOW_BYPASS=1 FLEET_STATE=$S fleet swarm dispatch \
     --task 'add a --version flag' --repo $R --agent stub
dispatched task=add a --version flag agent=stub checked=5 total=5 artifact=0094d2237...
```

Ledger now contains 3 lane_status rows via the plain path (previously wrote 0):

```
seq 1 lane builder state queued
seq 2 lane builder state running
seq 9 lane builder state passed
```

Via `--role builder` (with `FLEET_METER_WINDOWS="codex=200000,claude=100000"`):

```
seq 0 lane lead     queued
seq 1 lane designer queued
seq 2 lane builder  queued    (routed)
seq 3 lane verifier queued
seq 4 lane meter    queued
seq 6 lane builder  queued    (execute-path queued)
seq 7 lane builder  running
seq 10 lane builder failed
```

Validator: `lanes: valid (checked=8, total=8)`, exit 0.

## Change 3 — the contract is a projection (was: `ledger_ref` in the body, unreachable)

`contracts/lane-status.v1.json` rewritten as a **projected object** assembled from the authored
body plus immutable envelope stamps (`ts_wall`, `actor`, `ledger_ref{seq, hash}`) applied after
append. `required` is the union; the description states this is the projected view, not the
receipt body. `fleet contract lane-status validate` re-derives the stamps and validates every
row; `checked=0` fails (ExitCode 8). Governed by `docs/adr/ADR-0001-lane-status-projection.md`.

## Change 4 — validator + non-vacuous tests

`fleet contract lane-status validate` (new subcommand) plus pure, env-free field-level
validator `validate_lane_status_projection` and row validator `validate_lane_status_rows`
(publishes `checked=…, total=…` and yields `Err(ExitCode 8)` on `checked=0`).

**5 unit tests (all pass):**
- rejects an authored body missing `ledger_ref`
- accepts a stamped projection
- rejects invalid state / invalid hash
- row validator publishes the denominator and rejects zero input
- `refresh` tests (2 above) prove incremental + rewrite recovery

## Verification

```
$ cargo test -p fleet --bin fleet        # COMPLETE totals, bin suite
test result: ok. 103 passed; 0 failed; 1 ignored   (was also 1 ignored pre-change)

$ cargo test -p fleet                               # full package
test result: ok. 8 passed   (doc-tests etc.)
test result: ok. 103 passed; 0 failed; 1 ignored
test result: ok. 1 passed
test result: ok. 1 passed
```

`tests/acceptance/*`, `keel/fleet/src/route.rs`, `crew/crew/adapters/` untouched.

## verify.sh (FLEET_MUTANTS=0), uncontended

```
-- 17 passed, 1 failed, 1 skipped (denominator: 19 stages) --   VERIFY_EXIT=6
```

The 1 red is `corpus` C2 TIMEOUT-PERSISTENT — the pre-existing documented blocker
(`DENOMINATOR checked=34 total=34 excluded=69 caught=8 timeout_persistent=1`, identical to the
S2b/B13/B15 baseline). Not green, so no commit attempted (pre-commit hook execs the full suite and
would block on the same C2 red — established empirically in B13); backlog entries stay `[~]`.

Two verify stages tripped by this session's own polish and were fixed:
- **fmt** — `cargo fmt -p fleet` (2 diff hunks in console.rs, several in main.rs).
- **clippy -D warn** — redundant closure `|| state_dir()` → `state_dir`; 5×
  `format!("{}", lane_body(..))` → `&lane_body(..)`.
- **acceptance G1/G3 false-fail** — caused by a stray `keel/target-shared` (1.7 G) I created by
  exporting `CARGO_TARGET_DIR=$PWD/target-shared` from *inside* `keel/`. Its trybuild fixture
  `Cargo.toml`/`main.rs` sits under `keel/`, so p0.sh's `LIVE_MANIFEST` picked the fixture (no
  blake3 → G1) and `NMAIN` counted the second main.rs (→ G3). Removed the stray gitignored build
  dir; the target-shared cache is meant to live at the **repo root**, not inside `keel/`.