# Graph Node Audit — exhaustive, node-by-node

Date: 2026-09-09. Machine: this dev box (rustc 1.98.0, cargo 1.98.0, clippy 0.1.98). All commands
run from `/Users/rachitsrivastava/youtube/Principal Engineering/Light/fleet` unless stated.
`FLEET_LOAD_FACTOR=10000` used for every `fleet` invocation that touches the capacity gate (this
machine's load average was measured over 8×2=16 during parts of this audit); every use is called
out below. Exit codes captured as `cmd > f 2>&1; echo $?` — never read after a pipe.

## Denominators

- **Crates audited: 16 / 16** (all workspace members — `cargo test -p` + `cargo clippy -p` run for
  every one). `fleet-crew` is NOT a 17th Rust crate: `crates/fleet-crew/` holds a `pyproject.toml` +
  `uv.lock` (a Python project), has no `Cargo.toml`, and is correctly absent from
  `[workspace].members` — it is excluded tooling, not a dead Rust node.
- **Pipeline stages audited: 8 / 8** (Event, Classify, Scan, Plan, Dispatch, Verify, Merge, Teach —
  read from `src/pipeline/stage.rs`'s `PipelineStage::ALL`, not guessed).
- **Stages actually observed running in a real `fleet run`/`__pipeline_probe` invocation: 8 / 8.**
  Event/Classify/Scan/Plan/Dispatch/Verify/Teach were observed in a real `fleet run --repo
  <scratch> --task ...` (verify failed for real reasons on the trivial scratch repo, which starved
  the run before Merge). Merge was separately observed reached and PASSING via the hidden
  `__pipeline_probe` command (real code path, `NO_GATES` only skips the Verify gate table — Event
  through Teach all still execute for real). This is the same `run_pipeline` function `fleet run`
  calls; the only difference is which `&[GateSpec]` slice is passed to Verify.

## Part A — crate graph

Ledger of `cargo test -p <crate>` / `cargo clippy -p <crate> --all-targets -- -D warnings`, run
individually per crate (raw logs kept in `/tmp/audit/test_*.txt`, `/tmp/audit/clippy_*.txt` for
this session). Reachability = `grep -rl '<crate_snake>' src/ crates/ --include='*.rs'` excluding
the crate's own directory, cross-checked against every other crate's `Cargo.toml`.

| crate | tests (pass/fail, exit) | clippy exit | files (max lines) | reachable by | depends on (fleet-*) |
|---|---|---|---|---|---|
| fleet-types | 33/0, exit 0 | 0 | 14 (max 71) | everything below | — |
| fleet-lifecycle | 23/0, exit 0 | 0 | 17 (max 68) | `src/pipeline/{stages_dispatch,channels}.rs`, `src/dispatch/lifecycle_cmd.rs` | fleet-types |
| fleet-store | 28/0, exit 0 | 0 | 33 (max 79) | `src/pipeline/event_stage.rs`, `src/dispatch/ledger_cmd.rs` | fleet-types |
| fleet-events | 15/0, exit 0 | 0 | 16 (max 80) | **NONE** — see Finding S1 | fleet-types |
| fleet-router | 11/0, exit 0 | 0 | 9 (max 71) | `src/pipeline/{classify_stage,stages_dispatch,graph,event,ctx}.rs`, `src/dispatch/{route_cmd,error,role_cmd,run_cmd}.rs`, fleet-govern (failover/loop_types/loop_outcome) | fleet-types |
| fleet-scan | 22/0, exit 0 | 0 | 13 (max 61) | `src/pipeline/stages.rs`, `src/dispatch/{sow_probes,memory/adapter,memory/mod,plan_cmd}.rs`, `src/tests/sow_ambiguity_reads_content.rs` | fleet-types |
| fleet-plan | 54/0, exit 0 | 0 | 56 (max 76) | `src/pipeline/{teach_stage,stages}.rs`, `src/dispatch/plan_cmd.rs` | fleet-types |
| fleet-context | 31/0, exit 0 | 0 | 27 (max 80) | `src/dispatch/{context_cmd,walk}.rs` | fleet-types |
| fleet-verify | 32/0, exit 0 | 0 | 17 (max 74) | `src/pipeline/{graph,verify_stage,ctx}.rs`, `src/dispatch/{verify_runner_bounded,error,verify_cmd_tests,verify_ports,verify_cmd,run_cmd}.rs`, `src/print/verify_report.rs` | fleet-types |
| fleet-merge | 20/0, exit 0 | 0 | 9 (max 80) | `src/pipeline/{merge_stage,event}.rs`, `src/dispatch/{error,ledger_cmd}.rs`, fleet-worker (request/spawn/*) | fleet-types |
| fleet-memory | 17/0, exit 0 | 0 | 14 (max 80) | `src/dispatch/{sow_probes,memory/*}.rs` (7 files) | fleet-types |
| fleet-govern | 18/0, exit 0 | 0 | 22 (max 77) | `src/dispatch/{error,meter_cmd}.rs` | fleet-types, fleet-router |
| fleet-stream | 21/0, exit 0 | 0 | 24 (max 68) | **NONE** — see Finding S1 | fleet-types |
| fleet-worker | 43/0, exit 0 | 0 | 48 (max 80) | `src/dispatch/{agent_cmd_error,agents_cmd,worker_cmd,error,swarm_cmd,agent_cmd,spawn_probe_cmd,agent_cmd_run}.rs`, `src/tests/agent_child_dispatch.rs`, `src/cli/{args_agent,root}.rs` | fleet-types, fleet-merge |
| fleet-judge | 8/0, exit 0 | 0 | 11 (max 60) | `src/dispatch/adjudicate_{cmd_tests,cmd,render,cmd_error}.rs` | (none — standalone) |
| fleet-cli (`src`) | 87/0, exit 0 | 0 | 120 (max 80) | is the binary; depends on all 15 crates | all 15 |

Totals: **16/16 crates pass `cargo test -p`** (all exit 0, zero failed tests across ~460 total
test cases), **16/16 pass `cargo clippy -p --all-targets -- -D warnings`** (all exit 0, zero
warnings). **No file in any crate exceeds 80 lines** (checked with `find ... -exec wc -l`; several
sit exactly at the 80-line gate the repo's coding rubric enforces).

**DAG**: `fleet-types` is the only crate every other crate depends on (directly or transitively);
`fleet-judge` has zero `fleet-*` dependencies (fully standalone); `fleet-govern` is the only crate
that depends on another mid-layer crate (`fleet-router`); `fleet-worker` depends on `fleet-merge`.
Full edge list captured from every crate's own `Cargo.toml`:

```
fleet-lifecycle -> fleet-types
fleet-store     -> fleet-types
fleet-events    -> fleet-types
fleet-router    -> fleet-types
fleet-scan      -> fleet-types
fleet-plan      -> fleet-types
fleet-context   -> fleet-types
fleet-verify    -> fleet-types
fleet-merge     -> fleet-types
fleet-memory    -> fleet-types
fleet-govern    -> fleet-types, fleet-router
fleet-stream    -> fleet-types
fleet-worker    -> fleet-types, fleet-merge
fleet-judge     -> (none)
fleet-cli       -> all 15 above
```

**No cycle.** Clean layered DAG, `fleet-types` at the base, `fleet-cli` at the apex.

### DEAD crates (declared, zero non-self reachability)

- **`fleet-events`**: `src/Cargo.toml` declares `fleet-events = { path = "../crates/fleet-events" }`,
  but `grep -rn "fleet_events" src crates --include='*.rs'` (excluding the crate's own dir) returns
  **zero hits**. Nothing in `fleet-cli`'s `main.rs` (`mod cli; mod dispatch; mod pipeline; mod
  print; mod runtime;`) or any other crate ever writes `fleet_events::`. It compiles and its own 15
  tests pass, but nothing outside itself calls it.
- **`fleet-stream`**: same story. `src/Cargo.toml` declares it; `grep -rn "fleet_stream" src crates
  --include='*.rs'` (excluding its own dir) returns **zero hits**. Its own 21 tests pass in
  isolation; nothing imports it.

Both are real, tested, clippy-clean crates that are simply never wired into the binary that
depends on them — a `Cargo.toml` dependency line with no corresponding `use`. This is the same
defect class the brief names for `fleet-memory`/`fleet-judge`, except both of *those* are now
reachable: `fleet-memory` is used by 7 files under `src/dispatch/memory/` and `src/dispatch/
sow_probes.rs`, and `fleet-judge` is used by 4 files under `src/dispatch/adjudicate_*`. Whatever
made those two reachable did not touch `fleet-events`/`fleet-stream`.

## Part B — pipeline graph

Real stage list, from `src/pipeline/stage.rs::PipelineStage::ALL` (8 stages, not the 6 named in an
old error message): **Event → Classify → Scan → Plan → Dispatch → Verify → Merge → Teach**. `Teach`
is a required trailer (runs after every attempt, success or failure — see `graph.rs::run_pipeline`)
rather than a normal graph node; every other stage's only failure successor is `Teach`
(`allowed_successors`, asserted exhaustively in `stage.rs`'s own test).

Scratch repo used: `/tmp/audit/scratch_repo` — real `git init`, one committed Rust file
(`src/lib.rs` with an `add()` fn and a passing unit test), plus a staged second commit so `Merge`
has real staged files to count.

| stage | what it does (from code) | reached in real run? | real or stub? | ledger/stream emission | evidence |
|---|---|---|---|---|---|
| Event | `event_stage.rs`: creates `state_dir`, opens `fleet_store::Ledger`, appends a real `ReceiptEvent::RunStart` row | YES | REAL | YES — observable | `fleet run --repo /tmp/audit/scratch_repo --task "add a helper function" --json`, `PASS stage event (0.01s)`; `.fleet-state/ledger.chain` gained a new row with `"event":"run_start","body":{"task_id":"add a helper function"}`; `fleet ledger --verify` → `verified: 6/6` |
| Classify | `classify_stage.rs`: maps task text → `TaskClass`, runs real `fleet_router::decide` | YES | REAL | n/a (in-memory decision returned in `PipelineOutcome.classification`) | same run, `PASS stage classify (0.00s)`; JSON output shows the real `Decision` object incl. a genuine refusal ("role has no eligible tier set") |
| Scan | `stages.rs::scan()`: calls the real `fleet_scan::merge_questions(Vec::new())`, discards result | YES | **STUB IN CONTEXT** — real fn, fake (empty) input, result thrown away (`let _ =`) | NO | `merge_questions` with an empty candidate vec always returns `Assessment::Clear` (read `crates/fleet-scan/src/merge.rs:23-46`) — this stage can never do anything but trivially "pass"; it never actually scans the target repo for anything |
| Plan | `stages.rs::plan()`: calls real `fleet_plan::assemble_acceptance_checks_draft("wired-by-fleet-cli")`, discards result | YES | **STUB IN CONTEXT** — pure string template render, `let _ =` discards it | NO | `assemble_acceptance_checks_draft` (`crates/fleet-plan/src/lld_draft.rs:20-28`) is `format!()` of a fixed 4-check template parameterised only by the model name string; nothing about the target repo or task feeds in; the doc comment on `stages.rs` itself admits this ("minimal/no-op inputs...rather than the repo-specific inputs a real task run needs") |
| Dispatch | `stages_dispatch.rs`: real `fleet_router::decide`, real `fleet_lifecycle::resume("Building", ...)`, pushes through a real bounded channel (`blueprint_q`/`enqueue_build`), reads it back and checks task-id match | YES | REAL (exercises router+lifecycle+channel plumbing) but note: does not actually spawn a builder process/agent — that is `fleet-worker`'s job and this stage never calls it | NO direct ledger write | `PASS stage dispatch (0.00s)` in every run |
| Verify | `verify_stage.rs`: resolves the gates root, runs the real committed `fleet_verify::GATES` table (unit tests, mutants, semgrep, trivy, recur, detector-integrity, policy, corpus) through `RealRunner`/`WhichProbe` | YES | REAL, and genuinely fails on real defects (see Finding S1 below for a scoping bug in what it verifies) | NO ledger write, but stdout/stderr per-gate lines are the observable channel (`stage_report::started/finished`) | Run 1 (cwd = fleet workspace, `FLEET_VERIFY_BUDGET_SECS=120`): `cargo test`/`cargo mutants` ran for real and killed on timeout budget, rest budget-starved, exit 7. Run 2 (cwd = scratch repo, `FLEET_VERIFY_BUDGET_SECS=90`): unit tests/semgrep/detectors/policy genuinely PASSED, mutants/trivy/recur/corpus genuinely FAILED for real reasons (`Unparseable`, `NonZeroExit(1)`, `NonZeroExit(3)`, timeout) — this is real work being done, not a stub, but see S1: it verified the wrong tree |
| Merge | `merge_stage.rs`: real `git -C <repo> rev-parse --is-inside-work-tree` / `diff --cached --name-only` / `symbolic-ref --short HEAD`, feeds real counts into `fleet_merge::check_stage_nonempty` | YES (via `__pipeline_probe`, which only swaps Verify's gate table to empty — Merge's own code path is identical to what `fleet run` would execute) | REAL | NO direct ledger write | `FLEET_LOAD_FACTOR=10000 fleet __pipeline_probe --repo /tmp/audit/scratch_repo --task-id "merge-reach-test-2"` → stderr shows `PASS stage merge (0.03s)`, exit 0. Not reachable via plain `fleet run` on this scratch repo within the audit's time budget because Verify (the stage before it) genuinely failed real gates first — that is Verify's fault, not Merge's; distinguishing "stage did not run" from "stage ran, upstream failed" per the brief's own instruction |
| Teach | `teach_stage.rs`: on failure, builds a real `Lesson` via `fleet_plan::derive_lesson`, then **discards it** (`let _lesson = ...`); no-op on success | YES (every run reaches it — required trailer) | **STUB** — computes a real value, never persists or emits it anywhere | NO — confirmed no ledger append, no file write, no stream event; `derive_lesson` (`crates/fleet-plan/src/teach.rs:46`) is a pure data-transform fn with zero I/O, and its result is bound to `_lesson` and dropped | Every failing run (Run 1, Run 2, bad-repo run) reaches Teach (`final_stage: "Teach"` in every JSON output) but produces no artifact anywhere under `.fleet-state/` or elsewhere — `grep -r "Lesson\|lesson" .fleet-state/` finds nothing |

## Findings, ranked

**S1 — Verify does not verify `--repo`; `gate`/`oracle` have no `--repo` flag at all.**
`src/dispatch/verify_runner_bounded.rs::run_bounded` spawns every gate command with
`Command::new(bin).args(rest)` and **no `.current_dir(...)` call anywhere in the file or its
caller** (`grep -rn current_dir src crates --include='*.rs'` shows it used only for git subprocess
calls, `fleet-worker`'s worktree spawns, and test helpers — never for a verify gate). Every gate
that shells out (`cargo test --workspace`, `cargo mutants`) therefore runs against whatever
directory the `fleet` process's OS cwd happens to be, not the `--repo` path the caller passed to
`fleet run`. Repro:
- `cd "<fleet workspace>" && FLEET_LOAD_FACTOR=10000 FLEET_VERIFY_BUDGET_SECS=120 cargo run -p
  fleet-cli --bin fleet -- run --repo /tmp/audit/scratch_repo --task "add a helper function"
  --json` — the `cargo test --workspace` gate spawns and compiles/tests the **entire fleet
  workspace** (hyper, h2, tokio, ...), confirmed by `ps aux` during the run showing `rustc
  --crate-name hyper .../cargo-mutants-fleet-*.tmp` and `cargo test --no-run ... --package=fleet-
  cli@0.1.0 --package=fleet-context@0.1.0 ...` — not the one-file `scratch` crate `--repo` pointed
  at.
- `fleet gate --help` and `fleet oracle --help` show **neither subcommand accepts a `--repo` flag
  at all** — `error: unexpected argument '--repo' found` when passed one. These commands are
  hard-wired to the calling process's cwd, full stop.
- `semgrep-gate.sh` makes this explicit in its own comment: `ROOT="$(cd "$(dirname "$0")" && pwd)"`
  then `cd "$ROOT"` — it scans the **materialized gates-root** (a temp copy of the gate scripts
  themselves), never the target repo, regardless of anyone's cwd. Quoting the script: *"ROOT is
  therefore the gates root itself, and `.` below scans THAT tree, not necessarily a live fleet/
  checkout."*
- Net effect: of the 8 committed gates, only the two `cargo` gates are cwd-sensitive (and thus
  accidentally correct only when the operator happens to `cd` into `--repo` first, which `fleet
  run`'s own `--repo` flag gives no indication is required); the other 6 (semgrep/trivy/recur/
  detectors/policy/corpus) never look at `--repo` under any circumstances. This is exactly the "a
  proxy is not the property" failure this repo's own `PRINCIPLES.md` names as its most expensive
  lesson, now found inside the tool meant to enforce it.

**S2 — Teach computes a real Lesson and throws it away; nothing is ever taught.**
`src/pipeline/teach_stage.rs::teach` builds a `fleet_plan::Lesson` from the failing stage/error via
`derive_lesson`, binds it to `_lesson`, and returns. `derive_lesson` (`crates/fleet-plan/src/
teach.rs:46`) is a pure function with no I/O. No caller of `teach()` (`graph.rs::run_pipeline`)
does anything with the return value either — `teach()`'s signature is `-> ()`. Every failing
pipeline run in this audit (3 of them) reached the `Teach` stage and produced zero observable
artifact: no ledger row, no file under `.fleet-state/`, no stdout/stderr line naming the lesson's
content (only the raw `PipelineError` is printed by the CLI's own error path, which existed before
`Teach` ran). The stage's own doc comment claims it replaced "a fabricated lesson" with a real one
derived from the actual failure — true as far as it goes, but the real lesson is then discarded, so
the observable behavior (nothing persisted) is identical to the "fabricated lesson" version it
replaced.

**S3 — Scan and Plan stages are real functions called with fake/empty inputs, permanently.**
`src/pipeline/stages.rs::scan()` calls `fleet_scan::merge_questions(Vec::new())` — an empty
candidate list always yields `Assessment::Clear` by construction (read the fn: filter, sort,
dedup, truncate over an empty vec is trivially empty). `plan()` calls
`fleet_plan::assemble_acceptance_checks_draft("wired-by-fleet-cli")` — a fixed-template string
render with no task/repo input — and discards the result. Both stages' own file-header doc comment
admits this ("Scan/Plan call their crate's real orchestration fn with minimal/no-op inputs ... not
this composition layer's call to invent"). They are honestly flagged in the source, but the effect
on a real `fleet run` is that Scan and Plan can never observe or report anything about the actual
task/repo — they PASS unconditionally, every time, regardless of input. Distinguish from S1/S2:
these are declared/flagged deviations in the code itself, not silently broken; still, "stage always
trivially passes" is the exact defect class the brief asked to be named plainly.

**S4 — Seven CLI subcommands are declared and dispatch-wired but return NOT IMPLEMENTED verbatim.**
`fleet --help` lists `skills`, `attest`, `pr`, `contract`, `freeze`, `console`, `mcp` with their own
help text literally reading `"NOT IMPLEMENTED: ..."` (e.g. `pr: NOT IMPLEMENTED: fleet-merge has no
pr-emit fn exposed yet`). Not part of the crate graph or the 8-stage pipeline graph proper (the
brief's two explicit readings of "the graph"), but they are graph nodes in the CLI's own dispatch
table and are honestly labelled rather than silently broken — noted here for completeness since the
standing instruction is to check every node, and a command surface is a node set too.

## The one-line answer

**No — not every node of the graph works.** All 16 crates build, test, and clippy clean, and 7 of
the 8 pipeline stages are reachable with real logic behind them, but: `fleet-events` and
`fleet-stream` are dead crates (declared dependencies of `fleet-cli`, zero references anywhere
outside themselves); the `Verify` stage does not verify the repo `fleet run --repo` was pointed at
(S1 — it verifies the operator's cwd for its two `cargo` gates and the gates-root itself for the
other six, never `--repo`); `Teach` computes a lesson and discards it, teaching nothing (S2); and
`Scan`/`Plan` are wired to always-empty inputs so they can only ever trivially pass (S3, self-
flagged in the source). `Merge` itself is real and was confirmed passing via `__pipeline_probe`
where a real `fleet run` could not reach it in the time budget because `Verify` legitimately failed
first on real gate findings.
