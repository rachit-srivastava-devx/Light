# BLUEPRINT — `fleet-cli` (`src/`, not `crates/`)

> This unit is the RUNNING/COMPOSITION layer (MIGRATION-PLAN §2/§3 row "src/ (fleet-cli)"). It is a
> **binary**, not a library crate — there is no `pub` API for other crates to depend on, so §3 below
> gives the CLI surface + the pipeline-wiring module signatures instead of a `lib.rs` contract. Build
> branch: **refactor**. Precedence: this blueprint wins on implementation detail inside `src/`;
> `MIGRATION-PLAN.md` wins on crate boundary/DAG position. See `../_TEMPLATE.md` for the
> template this file adapts, and `../fleet-router/BLUEPRINT.md` for the rigor bar.

---

## 1. Header

- **Unit:** `fleet-cli` (lives at `src/`, MIGRATION-PLAN §2 layout — NOT `crates/fleet-cli/`)
- **One-line purpose:** Parse the operator's CLI invocation, load config, stand up the tokio +
  rayon runtimes, and wire the durable pipeline graph (`event → classify → scan → plan → dispatch →
  verify → merge → teach`) by calling into every `fleet-*` crate — it contains no decision logic,
  no hashing, no subprocess-spawning business rules, and no state-machine transitions of its own.
- **Build branch:** `refactor` (MIGRATION-PLAN §3, last row) — reshape `fleet/keel/fleet/src/main.rs`
  (6460 lines, hand-rolled dispatcher) and `swarm.rs` (910 lines, multi-lane dispatch) out of one
  god-binary into the composition root plus 14 `fleet-*` crates. **Caveat, read §5/notes: main.rs is
  not "mostly CLI with some logic embedded" — it is mostly business logic wearing a CLI face. This
  blueprint only claims the arg-parsing/printing/wiring shell; everything else is flagged to move.**
- **Imports (compile-time, workspace-path deps on every crate in the roster):** `fleet-types`,
  `fleet-lifecycle`, `fleet-store`, `fleet-events`, `fleet-router`, `fleet-scan`, `fleet-plan`,
  `fleet-context`, `fleet-verify`, `fleet-merge`, `fleet-memory`, `fleet-govern`, `fleet-stream`,
  `fleet-worker`. `fleet-crew` is a **runtime** dependency of `fleet-worker` (subprocess) reached
  transitively, not a direct import here (MIGRATION-PLAN §3 note).
- **Imported by:** none — this is the terminal node of the DAG (`types → {lifecycle, store} →
  {siblings} → src/`); nothing depends on `fleet-cli`.

## 2. Responsibility & non-goals

**Owns:** CLI argument parsing and the subcommand tree (today hand-rolled string matching in
`dispatch()`, `main.rs:57-252`; this blueprint replaces it with `clap`'s derive API, since `clap =
{ version = "4", features = ["derive"] }` is already a declared dependency in
`fleet/keel/fleet/Cargo.toml:8` yet unused — the derive macros were never actually adopted, only the
crate was pulled in); config loading (env + file precedence via `figment`, replacing the ad hoc
`env::var("FLEET_...")` reads scattered through `main.rs`/`route.rs`/`meter.rs`); building the tokio
runtime and the rayon global thread pool; computing the lane concurrency cap; wiring the two
channels `blueprint_q` → (on `Locked`) → `build_q`; declaring the pipeline's stages as a durable
Restate workflow (`restatedev/sdk-rust`) so a crash resumes mid-pipeline instead of restarting; and
translating each crate's typed return value into this process's exit code + human/JSON stdout
formatting. It is the ONLY place `std::process::exit`, `clap::Parser::parse`, and the Restate
service registration may appear.

**Non-goals (the seam — every one of these is a real function that exists TODAY in `main.rs` or
`swarm.rs` and must be physically moved, not just "considered someone else's job" — see §5):**
- Does **not** compute a routing decision — `route::command`'s arg-parsing shell stays here, but the
  6-stage filter (`decide`) is `fleet-router`'s (already blueprinted, `../fleet-router/`).
- Does **not** hash or append ledger receipts — `append_receipt` (blake3 chain, `main.rs:4355-4429`)
  and `ledger_rows`/`read_rows`/`verify_rows`/`ledger_dump` (`main.rs:4302-4619`) are `fleet-store`'s.
- Does **not** spawn/supervise agent subprocesses, manage fd-3, or run the builder/verifier
  processes — `spawn_agent_with_args` (`main.rs:3040-3157`, the `libc::dup2` fd-3 protocol),
  `run_with_evidence`/`run_verifier`/`verifier_for` (`main.rs:1611-2124`), and
  `run_freelane_agent`/`send_agent_packet` (`main.rs:3350-3455`) are `fleet-worker`'s.
- Does **not** create/remove git worktrees or decide the lane concurrency **execution** (only the
  cap **number** is computed here, as a pipeline-wiring parameter) — `worktree::lane_cap()`
  (`worktree.rs:169-174`) and the `thread::scope`-per-batch execution in
  `run_role_lanes_concurrently`/`run_one_role_lane` (`main.rs:1042-1188`) are `fleet-worker`'s;
  `git worktree add/remove` itself is `fleet-merge`'s (MIGRATION-PLAN §3 row 9).
- Does **not** run the oracle/verifier acceptance suites or the O1/O2 adjudication table —
  `oracle_command_inner`/`run_oracle`/`adjudication_table`/`adjudicate_command_inner`
  (`main.rs:2287-2578`) are `fleet-verify`'s.
- Does **not** roll back a landed artifact or reverse a git diff — `rollback_command`/
  `rollback_repo`/`rollback_artifact`/`reverse_landed_artifact`/`heal_applied_artifact`
  (`main.rs:2621-2977`) are `fleet-merge`'s.
- Does **not** build or emit a PR body, or run the git-apply/rev-parse sequence — `pr_emit_run`/
  `build_pr_body`/`git_ok` (`main.rs:5265-5665`) are `fleet-merge`'s.
- Does **not** measure token quota, cooldowns, or meter state — the whole `mod meter` (`main.rs:6460`
  onward, and its `meter::command` entry at `main.rs:60`) is `fleet-govern`'s.
- Does **not** drive the `Task<S>` lifecycle state machine — `lifecycle::command`/`drive_run`
  (imported from `fleet::lifecycle`, called at `main.rs:90,115`) belongs to the already-blueprinted
  `fleet-lifecycle`; this layer only forwards parsed args to it.
- Does **not** parse source with tree-sitter or build the symbol graph — `graph::graph_command`/
  `impact_command` (`main.rs:198-199`, backed by `graph.rs`) is `fleet-context`'s.
- Does **not** render the operator TUI's frames or own its data model — `console::run`
  (`main.rs:180-193`, backed by `console.rs`) is `fleet-stream`'s; this layer only parses
  `fleet console [--task ID]`'s args and forwards them.
- Does **not** implement MCP tool serving — `mcp::manifest_for_lease`/`mcp::serve` (`main.rs:206-219`,
  backed by `mcp.rs`) is `fleet-worker`'s (it exposes worker-owned tools) or `fleet-context`'s
  (symbol-graph tools) depending on which manifest section is being served; this layer only parses
  `fleet mcp [manifest] <lease>` and dispatches.
- If a future change needs this layer to compute a hash, spawn a process for anything other than
  starting the tokio/rayon runtimes themselves, or hold any state across two invocations, that is
  itself a defect — push it into the crate that owns that concern.

## 3. Public API contract

> No external crate imports this binary, so this "contract" is the CLI surface (must remain
> byte-compatible with today's real subcommands, enumerated in §5) plus the pipeline-wiring module's
> internal signatures — the part a reviewer needs to verify the graph is wired correctly and the
> concurrency cap is computed correctly. Real, `cargo check`-able Rust; typed errors throughout.
>
> **As built:** `restate-sdk` was never added as a dependency (§7) — every `restate_sdk::*` type
> below (`Context`, `HandlerError`, `#[restate_sdk::service]`) is illustrative of the pre-build
> design, not something the real `run_pipeline` imports. The real `pipeline::graph::run_pipeline`
> has the same stage-sequencing shape and the same crash-resume property, but takes a
> `pipeline::ctx::StageCtx` (backed by `pipeline::step_log::StepLog`, an on-disk resumable step
> log) instead of a Restate `Context`, and returns `Result<PipelineOutcome, PipelineError>`
> directly rather than wrapping the error in a `HandlerError`. See §7/§8 for the as-built shape.

```rust
//! fleet-cli — composition root. Parses args, builds the runtime, wires the pipeline graph, and
//! calls into fleet-* crates. Contains no decision logic, no hashing, no subprocess business rules.

use clap::{Parser, Subcommand};
use std::num::NonZeroUsize;

/// Top-level CLI, replacing the hand-rolled `match args.first()...` in `main.rs:57-252` with
/// clap's derive API (the dependency already exists, unused, at `Cargo.toml:8`). Every variant
/// name below is a REAL subcommand string from the current dispatcher (see §5's citation table) —
/// none invented, none dropped; `--print` and the bare-invocation-to-REPL behavior (`main.rs:43-49`)
/// are preserved as pre-parse steps in `main()`, not as clap subcommands (they mutate `args` before
/// clap ever sees them today, and must keep doing so — clap cannot represent "no subcommand given
/// AND stdin is a tty" as a subcommand).
#[derive(Parser, Debug)]
#[command(name = "fleet", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Meter(MeterArgs),
    Route(RouteArgs),
    Roles,
    Swarm(SwarmArgs),
    Sow(SowArgs),
    Plan(PlanArgs),
    Skills { check: bool },
    RoleCheck(RoleCheckArgs),
    Agents(AgentsArgs),
    Lifecycle(LifecycleArgs),
    Run(RunArgs),
    Oracle(OracleArgs),
    Adjudicate { artifact: String },
    Attest(AttestArgs),
    Pr(PrArgs),
    Status { json: bool },
    Rollback(RollbackArgs),
    Ledger(LedgerArgs),
    Contract(ContractArgs),
    Gate(GateArgs),
    Freeze(FreezeArgs),
    Console { task: Option<String> },
    Graph(GraphArgs),
    Impact(ImpactArgs),
    Mcp(McpArgs),
    Completions { shell: Shell },
    Doctor,
    Version,
}

/// Every field group above is a `#[derive(clap::Args)]` struct in `src/cli/*_args.rs` (§8) — elided
/// here for brevity; each field name matches an existing `--flag` cited in §5's evidence column.

/// The lane concurrency cap this process may run at once. Computed once at startup from three
/// independent ceilings and never recomputed mid-run (a config that changes cores mid-process is
/// out of scope — restart to pick up a new cap). This REPLACES `worktree::lane_cap()`
/// (`worktree.rs:169-174`, `cores.saturating_sub(2).clamp(1, 16)`) with the three-way min the task
/// brief specifies; `worktree::lane_cap()` itself becomes dead code once this ships (flagged in
/// the divergence note — MIGRATION-PLAN never named a `ram_lanes`/`review_cap` ceiling).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConcurrencyCap(NonZeroUsize);

impl ConcurrencyCap {
    /// `min(available_cores - 2, ram_lanes, review_cap)`, floored at 1. `available_cores` comes
    /// from `std::thread::available_parallelism()` (falls back to 1 on error, matching
    /// `worktree.rs:170-172`'s existing fallback). `ram_lanes` is the caller-supplied estimate of
    /// how many lanes fit in available RAM (injected, never read from `/proc` or similar directly
    /// in this function — the caller measures RAM and passes a count, so this fn stays pure and
    /// testable). `review_cap` is the hard ceiling on lanes awaiting Opus review at once (product
    /// constraint, default 3, overridable via config for a future review-capacity change).
    pub fn compute(available_cores: usize, ram_lanes: usize, review_cap: usize) -> Self {
        let cap = available_cores
            .saturating_sub(2)
            .min(ram_lanes)
            .min(review_cap)
            .max(1);
        Self(NonZeroUsize::new(cap).expect("max(1) guarantees nonzero"))
    }

    pub fn get(self) -> usize {
        self.0.get()
    }
}

/// One node in the fixed 8-stage pipeline graph. Order is significant and enforced by `PipelineGraph`
/// (§4) — a stage may only hand off to the next one in this list, or to `Teach` from any stage on
/// failure (failure teach-back is not skippable).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, serde::Serialize)]
pub enum PipelineStage {
    Event,
    Classify,
    Scan,
    Plan,
    Dispatch,
    Verify,
    Merge,
    Teach,
}

/// Why the pipeline could not advance past a stage. Every variant names the stage and is
/// constructed only by this layer's wiring code (each crate returns ITS OWN typed error; this enum
/// wraps those at the seam so the pipeline's `run` fn has one error type to propagate).
#[derive(Debug)]
pub enum PipelineError {
    Event(fleet_events::IngressError),
    Scan(fleet_scan::ScanError),
    Plan(fleet_plan::PlanError),
    Dispatch(fleet_router::Refusal),
    Verify(fleet_verify::VerifyError),
    Merge(fleet_merge::MergeError),
    /// The Restate journal/runtime itself faulted (distinct from any stage's own business error).
    Runtime(String),
}

/// Advance one durable pipeline run through every stage in order, using `ctx.run(...)`
/// (`ContextSideEffects`, restate-sdk 0.12) to journal each stage's outcome so a process crash
/// mid-pipeline resumes at the next unjournaled stage rather than re-running completed ones.
/// Registered as a Restate `#[handler]` inside a `#[restate_sdk::service]` (or `#[workflow]` for
/// the once-per-task-id semantics — see §4's "Restate service shape" row) impl in `pipeline/service.rs`.
pub async fn run_pipeline(
    ctx: restate_sdk::prelude::Context<'_>,
    event: fleet_events::IngressEvent,
    cap: ConcurrencyCap,
) -> Result<PipelineOutcome, restate_sdk::prelude::HandlerError> {
    unimplemented!(
        "see §4 'pipeline graph wiring' + §6 behavior spec: each stage is a `ctx.run(|| ...)` \
         closure calling exactly one fleet-* crate fn, in the order Event, Classify, Scan, Plan, \
         Dispatch, Verify, Merge, Teach; Dispatch fans lanes out through blueprint_q -> (on \
         Locked) -> build_q, bounded by `cap`, using tokio::sync::mpsc + a rayon scope for the \
         CPU-bound scan/verify stages"
    )
}

/// Terminal outcome of one pipeline run, returned to the CLI layer for exit-code mapping (§6).
#[derive(Debug, serde::Serialize)]
pub struct PipelineOutcome {
    pub task: fleet_types::TaskId,
    pub final_stage: PipelineStage,
    pub result: Result<(), PipelineError>,
}
```

## 4. Data model & invariants

| Type | Invariant enforced | Illegal state made unrepresentable |
|---|---|---|
| `ConcurrencyCap` | Always `>= 1` (three-way `min` then `.max(1)`) — never 0, which would mean "no lanes ever run" and silently hang the pipeline forever. | A misconfigured `ram_lanes=0` or a single-core box computing a 0-lane cap and deadlocking `blueprint_q`. |
| `PipelineStage` | Exactly 8 variants, and `PipelineGraph`'s edge table (private to `pipeline/graph.rs`) only allows `Stage(n) -> Stage(n+1)` or `Stage(n) -> Teach`; enforced by an exhaustive `match` with no wildcard arm, same discipline as `fleet-router`'s `role_allows`. | A stage silently skipping ahead (e.g. `Scan` handing off straight to `Merge`, bypassing `Plan`/`Dispatch`/`Verify`) without it being a compile-time-visible edge-table change. |
| `PipelineOutcome.result` | `Ok(())` iff every stage through `Merge` succeeded; `Teach` always runs regardless (success or failure) and is not part of the `Result` — it is a required trailer, not a conditional stage. | A run that "succeeded" but never wrote back a lesson (`fleet/PRINCIPLES.md`'s core discipline: a caught mistake with no lesson written is a repeat waiting to happen — MIGRATION-PLAN §7's teach-back log is this pipeline's Teach stage, automated). |
| `blueprint_q` / `build_q` (two `tokio::sync::mpsc::Receiver<LaneTask>` channels, `pipeline/channels.rs`) | An item enters `build_q` only after its producer observes `LifecycleState::Locked` for that task (checked via `fleet_lifecycle::Task<Locked>`'s marker type — a compile-time proof, not a runtime string compare); nothing can be enqueued to `build_q` directly. | A lane starting to build against a SOW/contract that was never locked — the exact class of bug D53 in `fleet/PRINCIPLES.md` names ("structural checks before the policy gate"), generalized to a type-level gate instead of an `if` check like `main.rs:95-109`'s repo/git-dir check. |
| `Cli`/`Commands` | Every variant maps 1:1 to a subcommand string that exists TODAY in `main.rs:57-252`'s `match` (§5); clap's derive enforces exhaustiveness and rejects unknown flags at parse time instead of the hand-rolled `_ => { eprintln!("fleet: unknown command..."); Err(EXIT_REFUSAL) }` fallback (`main.rs:247-250`). | A silently-typo'd subcommand reaching business logic instead of failing at parse time with clap's own usage message. |

**Money/precision:** no money type here — `Tokens` (integer minor-units) is `fleet-types`'s; this
layer never does arithmetic on tokens, only forwards parsed `--tokens`-style flags as opaque strings
or `u64` counts to `fleet-govern`.

**Clock/RNG/IO injection points:** this is the OUTERMOST layer, so unlike every inner crate it is
*allowed* to read the real clock, real env, real stdin/stdout, and spawn the real tokio runtime —
that is its job. The rule instead is: no *business logic* reads them directly. `SystemTime::now()`
(used today at `main.rs:4958` for `now_rfc3339`, and `main.rs:30`'s import) moves to `fleet-store`
(receipt timestamps) or `fleet-verify` (attestation timestamps); this layer's own direct clock/env
reads are limited to (a) building `figment`'s config from `env::var`/file, and (b)
`std::thread::available_parallelism()` for `ConcurrencyCap`'s `available_cores` input — both named
here, both wiring, neither a business decision.

**Restate service shape (pre-build design; not built — see §7/§8):** the 8-stage graph was designed
as one `#[restate_sdk::service]` (stateless dispatch — each pipeline run is independent, keyed by
`TaskId`, no cross-run shared state) rather than a `#[workflow]`/virtual object, because nothing in
the pipeline needs the "callable after completion" interaction semantics `WorkflowContext`/
`SharedWorkflowContext` add. **As built**, the same stateless-per-`TaskId` shape and the same
"journal each stage so a crash resumes at the next one" durability property are provided by
`pipeline::step_log::StepLog` (a resumable on-disk log keyed by `TaskId`, recording which
`PipelineStage`s have completed) instead of a Restate journal — `pipeline/graph.rs`'s own doc
comment states this is deliberate ("Restate deferred... per the task brief's explicit fallback")
and names what swapping in the real SDK later would require.

## 5. Reuse map

Source: `fleet/keel/fleet/src/main.rs` (6460 lines, function index read in full 2026-09-08) and
`fleet/keel/fleet/src/swarm.rs` (910 lines, read 2026-09-08) and `fleet/keel/fleet/src/worktree.rs`
(`lane_cap`, lines 169-174).

| Fleet source (file:line) | What it does today | Lift as-is? | Change needed |
|---|---|---|---|
| `main.rs:39-55` (`fn main`) | Parses raw `args`, handles `--print` and the bare-invocation→REPL rule, calls `dispatch`, maps `Result<(), i32>` to `process::exit`. | partial | Keep the `--print`/REPL pre-parse steps (clap cannot express "no args AND stdin is a tty"); replace the rest with `Cli::parse()` + `run_pipeline`/dispatch-table call; keep the exit-code mapping idea but source codes from each crate's typed error via a `From` impl, not a bare `i32`. |
| `main.rs:57-252` (`fn dispatch`, the hand-rolled `match args.first()...`) | String-matches ~28 real subcommands (`meter`, `route`, `roles`, `swarm`, `sow`, `plan`, `skills`, `role-check`, `agents list`, `lifecycle`, `run`, `oracle`, `adjudicate`, `attest verify`, `pr emit`, `status`, `rollback`, `ledger`, `contract`, `gate`, `freeze`, `console`, `graph`/`impact`, `mcp`, `completions`, `doctor`, `version`/`--version`/`-V`, `help`/`--help`/`-h`), plus test-only `__repl`/`__agent`/`__lanes_probe`/`__pr_emit_probe`. | **no — this is the exact god-function this blueprint replaces** | Becomes `Cli`/`Commands` (§3) parsed by clap derive; each arm's ARG-PARSING becomes one `src/cli/*_args.rs` struct; each arm's BUSINESS CALL becomes one `src/dispatch/*_cmd.rs` fn that calls the owning crate. Test-only subcommands (`__repl`, `__agent`, `__lanes_probe`, `__pr_emit_probe`) stay as hidden clap subcommands (`#[command(hide = true)]`) — they drive real production code through the real binary, per their own doc comments (`main.rs:170-177`), and that property must survive the refactor. |
| `main.rs:95-109` (structural-check-before-policy-gate, D53) | `--repo`/`--task` validity checked before `enforce_accepted_sow` — a documented lesson (D53) about ordering checks so the error a user sees can actually be fixed. | as inspiration | Port the ORDERING discipline into `dispatch/run_cmd.rs`'s arg-validation, but the checks themselves (repo-is-dir, repo-is-git) are IO — keep them here (composition root already touches the filesystem for CLI validation) rather than pushing to a crate; the SOW-acceptance check itself calls `fleet-plan`. |
| `main.rs:724-879` (`fn swarm_command`) | Dispatches `swarm status/bandwidth/allocate/complete/dispatch` by positional-arg pattern matching, then calls `swarm::{bandwidth,allocate,complete_task,load_or_seed}` and formats output. | partial | Arg-parsing → `SwarmArgs` (clap `Subcommand`); the `swarm::*` calls themselves are `fleet-worker`'s (agent registry/bandwidth) — this file's job shrinks to parse+call+print. |
| `main.rs:1042-1188` (`run_one_role_lane`, `run_role_lanes_concurrently`, `lanes_probe_command`) | Batches routed roles into `thread::scope` groups of `worktree::lane_cap()`, spawns each lane, joins every batch before starting the next. | **no — superseded** | This is the CURRENT concurrency mechanism the pipeline's `Dispatch` stage replaces. The batching-with-join-before-next-batch shape is sound but must move behind `ConcurrencyCap`/`blueprint_q`→`build_q` (§4) and a tokio+rayon scope instead of raw `thread::scope`, and the actual lane execution (`run_one_role_lane`'s worktree add/spawn/remove) is `fleet-worker`'s, not this layer's. See divergence note below — MIGRATION-PLAN's roster never named this as the thing the new pipeline supersedes. |
| `worktree.rs:169-174` (`lane_cap`) | `cores.saturating_sub(2).clamp(1, 16)` — cores-minus-2 only, no RAM or review-capacity ceiling, hardcoded 16 max. | **no — becomes dead code** | Replaced by `ConcurrencyCap::compute` (§3), which adds the `ram_lanes`/`review_cap=3` ceilings the task brief requires and that `lane_cap` never had. `fleet-worker` (which still needs a lane count for its own batching) should call `ConcurrencyCap` from this layer rather than keep a second, divergent cap function — flag for Opus: does `fleet-worker` import `fleet-cli`'s cap (a DAG-illegal edge, since nothing may depend on `src/`) or does `ConcurrencyCap` itself need to live in a shared crate (`fleet-govern`?) so both `fleet-cli` and `fleet-worker` can compute it? This blueprint cannot resolve that alone — see divergence notes. |
| `main.rs:1189-1465` (`swarm_dispatch_command`) | Parses `swarm dispatch --task/--repo/--agent/--role`, resolves a role via the router, runs the lane(s), records evidence. | partial | Arg-parsing stays; role resolution is `fleet-router`'s (already blueprinted); evidence recording is `fleet-store`'s; only the sequencing ("parse, call router, call worker, call store") stays here as the `Dispatch` pipeline stage's wiring. |
| `main.rs:2211-2270` (`state_dir`/`state_dir_quiet`/`valid_artifact_id`) | Resolves `$FLEET_STATE`, validates artifact-id shape. | partial | `$FLEET_STATE` resolution is config (→ `figment` source in this layer); `valid_artifact_id`'s validation predicate is a pure fn that could live in `fleet-types` (an ID-shape invariant, same family as `TaskId`/`NodeId`) — flagged as a possible `fleet-types` addition, not decided here. |
| `main.rs:4984-5057` (`print_help`), `5155-5214` (`print_completions`) | Static help text + `clap_complete`-shaped shell completions (note: `clap_complete = "4"` is ALSO already an unused declared dependency, `Cargo.toml:9`, same pattern as `clap`'s unused derive feature). | yes, as generated | Once `Cli` is a real clap `Parser`, `--help` and `fleet completions <shell>` are generated for free via `clap::CommandFactory`/`clap_complete::generate` — the hand-written text in `main.rs:4984-5214` becomes redundant and is deleted, not ported line-for-line (its CONTENT — which subcommands exist and what they do — is preserved via clap's own `about`/`long_about` attributes on each `Commands` variant). |
| `main.rs:57-64` (`meter`/`route` arms) | Shells to `meter::command`/`route::command(&args[1..], &state)`. | partial | Arg-parsing → `MeterArgs`/`RouteArgs`; `meter::command`'s body is `fleet-govern`'s; `route::command`'s body is `fleet-router`'s `decide` (already blueprinted) plus this layer's own printing (`fleet-router`'s blueprint §2 non-goals explicitly assign printing/receipt-writing to `src/` — this row is that assignment's other half). |
| `swarm.rs:1-910` (whole file) | Agent registry load/seed/persist, bandwidth/allocation math, lifecycle-milestone bumping. | **no** | This is `fleet-worker`'s (agent registry + bandwidth/allocation, matching MIGRATION-PLAN row 13's "crew adapters" framing) — none of it is CLI wiring; `swarm.rs` should not exist under `src/` post-refactor at all, only `src/dispatch/swarm_cmd.rs`'s thin arg-parse-and-call shell should. |

## 6. Behavior spec

### `fn ConcurrencyCap::compute(available_cores: usize, ram_lanes: usize, review_cap: usize) -> ConcurrencyCap`

| Input dimension | Behavior |
|---|---|
| empty | n/a — all three params are required `usize`, no "absent" state; a caller with no RAM measurement passes `usize::MAX` for `ram_lanes` to mean "unconstrained by RAM," never `0`. |
| null / `None` | n/a — not an `Option`-typed API; see "empty" row for the "unconstrained" convention. |
| wrong-type | n/a — all `usize`, no parsing at this boundary (parsing happens in config/CLI loading, upstream of this pure fn). |
| huge | `available_cores = usize::MAX` (e.g. a `available_parallelism()` misreport) → `saturating_sub(2)` avoids underflow, then `.min(ram_lanes).min(review_cap)` still bounds the result to whichever of the three is smallest — never panics, never returns an absurdly large cap as long as `review_cap` (default 3) is sane. |
| negative | not representable — `usize` is unsigned; a negative core/lane/review count cannot reach this fn. |
| duplicate | n/a — pure value fn, no identity to duplicate. |
| concurrent | Pure fn, no shared/interior-mutable state — trivially `Send + Sync`, safe to call from any number of threads. |
| unicode / non-ASCII | n/a — no string input. |
| already-exists | n/a — no persisted state; calling twice with the same inputs yields the same `ConcurrencyCap`, required for §9's determinism test. |
| partial-failure | n/a — no IO, cannot fail partially; always returns synchronously, never panics (see "huge" row: `saturating_sub` guards the one possible arithmetic fault). |
| zero (cap-specific: `available_cores <= 2`, or `ram_lanes == 0`, or `review_cap == 0`) | Any of the three driving the raw `min` to 0 is caught by the trailing `.max(1)` — the cap is NEVER 0; a single-core box or a review_cap misconfigured to 0 still returns a cap of 1, not a pipeline that can never dispatch a lane. This is the invariant named in §4's table and must be asserted directly in a unit test, not left implicit. |

### `async fn run_pipeline(ctx, event, cap) -> Result<PipelineOutcome, HandlerError>`

> As built, `ctx` is a `pipeline::ctx::StageCtx` (`StepLog`-backed) and the return type is
> `Result<PipelineOutcome, PipelineError>`, not `HandlerError` — see §3's as-built note. The
> per-row behavior below (crash resumption included) is unchanged in intent; only the
> journal/context type differs. `src/tests/pipeline_resumes_after_crash.rs` is the real test
> backing the "partial-failure" row's requirement to actually kill the process and assert resume.

| Input dimension | Behavior |
|---|---|
| empty | An `IngressEvent` with no actionable content (e.g. a github webhook ping, not a task-worthy event) is refused at `Classify` — `PipelineOutcome.final_stage = Classify`, `result = Err(...)`, and `Teach` still runs (logging "classified as noise, no lesson" is itself a valid, cheap teach-back entry, not a skip). |
| null / `None` | n/a at this boundary — `IngressEvent` is a typed enum from `fleet-events`, constructed only by that crate's adapters; no null/None can reach `run_pipeline` directly (a malformed webhook payload is rejected inside `fleet-events`, before this fn is ever called). |
| wrong-type | Not reachable — same reasoning; `fleet-events` owns payload deserialization and its own typed errors, never handing this layer a raw/untyped payload. |
| huge | A pipeline run producing thousands of lane tasks at `Dispatch`: bounded strictly by `cap.get()` concurrent lanes at any instant via `blueprint_q`/`build_q`'s bounded channel capacity — extra lane tasks queue, they do not spawn extra OS threads/processes beyond the cap, regardless of how many the `Plan` stage produced. |
| negative | n/a — no signed numeric input at this boundary. |
| duplicate | Two ingress events describing the same underlying task (e.g. a webhook retry) — de-duplication is `fleet-events`' job (idempotency key), not this fn's; `run_pipeline` itself is safe to invoke twice for the "same" event because Restate's journal makes re-invocation of an already-completed run a no-op replay, not a second execution (this is the exact property the Restate SDK exists to provide here). |
| concurrent | Multiple independent `run_pipeline` invocations (distinct `TaskId`s) run concurrently by design — the whole point of `ConcurrencyCap`; two invocations for the SAME `TaskId` concurrently is prevented by Restate's per-key single-flight semantics for a keyed service (documented in `pipeline/service.rs`, not re-implemented here). |
| unicode / non-ASCII | Task descriptions/repo paths flow through as opaque `String`/`PathBuf` — this layer performs no text processing on them, only forwards to the crate that does (`fleet-scan`, `fleet-plan`); no normalization, no panic. |
| already-exists | Re-running a `TaskId` that already reached `Teach` — Restate returns the already-computed `PipelineOutcome` from the journal rather than re-executing (replay semantics, same mechanism as "duplicate" above). |
| partial-failure | A crash between `Dispatch` completing and `Verify` starting: on restart, Restate replays journaled `ctx.run` results for `Event`/`Classify`/`Scan`/`Plan`/`Dispatch` (no re-execution, no double-dispatch of already-started lanes) and resumes at `Verify` — this IS the durability property the whole crate exists to provide; a test must actually kill the process mid-pipeline and assert resumption, not merely assert the code compiles (§9). |

## 7. Dependencies

> **As built** (`src/Cargo.toml`): `restate-sdk` was never added. Every file in `src/pipeline/`
> that would have used it carries an explicit "Restate deferred" doc comment (`pipeline/mod.rs`,
> `pipeline/graph.rs`, `pipeline/step_log.rs`) explaining the substitution — see the note after
> this table and §8's file-layout note.

| Crate | Version | Why |
|---|---|---|
| `clap` | `4` (features = `["derive"]`) | Derive-based `Cli`/`Commands` (§3), replacing the hand-rolled `match` at `main.rs:57-252`. |
| `clap_complete` | `4` | Generates `fleet completions <shell>` from the derived `Cli`, replacing the hand-written `print_completions` (`main.rs:5155-5214`). |
| `figment` | `0.10` (features = `["env"]`) | Layered config: env vars (`FLEET_STATE`, `FLEET_ROUTE_COOLDOWNS`, etc.) over an optional config file. Built with only the `env` feature — no `toml` file-source feature was added, so file-based config layering is narrower than the pre-build proposal's `["env", "toml"]`. |
| `tokio` | `1` (features = `["io-std","macros","rt","rt-multi-thread","sync","time"]`) | Async runtime for the pipeline; `time` was added beyond the pre-build proposal's feature list (used by the pipeline's own timing, not by a Restate host, since none was added). |
| `rayon` | `1` | CPU-bound stages (`Scan`, parts of `Verify`) run inside a rayon scope sized to `ConcurrencyCap`. |
| `thiserror` | `1` | Typed dispatch/pipeline error enums (`DispatchError`, `PipelineError`, `PlanAheadError`). Not named in the pre-build dependency table at all — added because this layer's own error types (§4/§6) need it, same as every other crate in the roster. |
| `serde` (features = `["derive"]`), `serde_json` | `1` | `PipelineOutcome`/dispatch JSON output (`print/json.rs`). |
| every `fleet-*` crate (§1 imports) | workspace-path (`{ path = "../crates/<name>" }`) | The actual work each pipeline stage delegates to. |

**Not added, confirmed absent from the built `src/Cargo.toml`:** `restate-sdk`. The durable
crash-resume property §4/§6 describe as a Restate journal is instead provided by
`pipeline::step_log::StepLog`, a resumable step log written to disk — see the note below and §8.
`pipeline/graph.rs`'s own doc comment states the intent explicitly: "Swapping in the real SDK later
means replacing this file's body with `ctx.run(..)` closures around the same `stages::*` calls."

## 8. Crate file layout

> **HARD RULE: every source file ≤ 80 lines.** Unlike `crates/*`, this lives at `src/` per
> MIGRATION-PLAN §2. `main.rs` today is 33 lines (the pre-refactor 6460-line god-file is gone).
>
> **As built** (`find src -name '*.rs' \| sort`, line counts via `wc -l`), the layout differs from
> the pre-build sketch in two structural ways, both intentional and documented in-code:
> 1. **No `pipeline/service.rs` / `restate_sdk::service` impl** — §7 already notes `restate-sdk`
>    was never added. Its place is taken by `pipeline/step_log.rs` (`StepLog`, a resumable on-disk
>    step log) plus `pipeline/ctx.rs` (`StageCtx`) and `pipeline/dispatch_table.rs`
>    (`dispatch_table::run_one`'s stage match) — `pipeline/graph.rs` wires these together instead of
>    hosting a Restate service. `pipeline/stages.rs`/`stages_dispatch.rs` and one file per stage
>    (`event_stage.rs`, `classify_stage.rs`, `verify_stage.rs`, `merge_stage.rs`, `teach_stage.rs`)
>    replace the single `graph.rs` "one fleet-* call per stage" sketch with one file per stage to
>    stay under the 80-line cap.
> 2. **`dispatch/` grew far beyond the pre-build sketch's 5 files** — real subcommand coverage
>    needed a dedicated file per command family (`agent_cmd*.rs`, `agents_cmd.rs`, `context_cmd.rs`,
>    `lifecycle_cmd.rs`, `meter_cmd.rs`, `plan_cmd.rs`, `planahead_cmd.rs`, `run_cmd.rs`,
>    `sow_probes.rs`, `spawn_probe_cmd.rs`, `worker_cmd.rs`, `walk.rs`, `error.rs`/`error_exit.rs`,
>    `verify_ports.rs`), plus a `src/pipeline/planahead/` module (the plan-ahead build-overlap
>    mechanism from `blueprints/REQUIREMENTS.md` #7/#8, not named anywhere in this blueprint) and a
>    `src/tests/` directory (this blueprint's `tests/` sketch below assumed a top-level `tests/`
>    dir; the real crash-resume/subcommand-parsing tests live under `src/tests/` instead, alongside
>    a `support/` module).
>
> `ops_cmd.rs` also confirms §2's non-goals table directly: `console`/`freeze`/`contract`/`pr`/
> `attest`/`adjudicate`/`skills` are stubbed behind `not_yet_implemented()` →
> `DispatchError::NotYetImplemented`, one named reason per subcommand citing which crate has no
> public entry point yet — this matches, not contradicts, this blueprint's own §2 non-goals table
> (those subcommands were never claimed as implemented here).

```
src/
  main.rs                     # 33  — pre-parse, Cli::parse(), build runtime, dispatch, exit
  cli/
    mod.rs                    # 7   — module decls + re-exports of Cli/Commands
    root.rs                   # 80  — Cli, Commands enum (§3)
    args_core.rs                # 62 — RunArgs/SwarmArgs and other core shared fields
    args_agent.rs                 # 30 — agent-related arg structs
    args_ctx.rs                    # 53 — context/graph/impact arg structs
    args_ops.rs                     # 48 — ops (status/mcp/freeze/...) arg structs
  runtime/
    mod.rs                     # 9   — re-exports
    tokio_rt.rs                 # 24  — builds the multi-thread tokio Runtime
    rayon_pool.rs                 # 23 — global rayon::ThreadPoolBuilder sized by ConcurrencyCap
    concurrency_cap.rs             # 72 — ConcurrencyCap + compute()/from_env() (§3/§4/§6)
    config.rs                     # 49 — figment-backed Config struct (env-only, no file source)
  pipeline/
    mod.rs                     # 22  — re-exports; documents "Restate deferred" (see §7)
    ctx.rs                      # 20  — StageCtx, the step-log-backed stand-in for restate's Context
    stage.rs                     # 79 — PipelineStage enum + edge-table match (§4)
    event.rs                      # 52 — PipelineOutcome, PipelineError (§3/§4)
    channels.rs                    # 48 — blueprint_q/build_q mpsc setup, Locked-gate enqueue fn
    dispatch_table.rs               # 22 — run_one(): matches a PipelineStage to its stage fn
    stages.rs                        # 29 — per-stage fn signatures/shared plumbing
    stages_dispatch.rs                # 29 — stage dispatch helpers
    event_stage.rs                     # 25 — Event stage
    classify_stage.rs                   # 29 — Classify stage
    verify_stage.rs                      # 27 — Verify stage
    merge_stage.rs                        # 44 — Merge stage
    teach_stage.rs                         # 17 — Teach stage (always runs last)
    graph.rs                                # 76 — run_pipeline(): wires stages via StepLog, not restate
    step_log.rs                              # 57 — StepLog: on-disk resumable step log (replaces restate's journal)
    planahead/
      mod.rs                                 # 12 — re-exports
      error.rs                                # 23 — PlanAheadError
      orchestrator.rs                          # 59 — run_plan_ahead(), UnitStep
      workers.rs                                # 56 — backpressured worker pool
      unit_log.rs                                # 49 — crash-resume durability for plan-ahead units
  dispatch/
    mod.rs                     # 68  — re-exports + dispatch table
    error.rs                    # 62 — DispatchError
    error_exit.rs                 # 24 — DispatchError -> process exit code mapping
    route_cmd.rs                   # 50 — `fleet route`: parse -> fleet_router::decide -> print
    swarm_cmd.rs                    # 30 — `fleet swarm ...` shell
    run_cmd.rs                       # 47 — `fleet run` shell
    plan_cmd.rs                       # 48 — `fleet plan`/sow shell
    planahead_cmd.rs                   # 35 — `fleet planahead` shell -> pipeline::planahead
    sow_probes.rs                       # 57 — SOW-ambiguity probe wiring
    spawn_probe_cmd.rs                   # 35 — `fleet __agent`/spawn-probe test-only shells
    agent_cmd.rs                          # 39 — `fleet agent` shell
    agent_cmd_run.rs                       # 79 — agent-run dispatch body
    agent_cmd_error.rs                      # 49 — agent-cmd-specific error mapping
    agents_cmd.rs                            # 19 — `fleet agents list` shell
    worker_cmd.rs                             # 14 — `fleet worker` shell
    context_cmd.rs                             # 29 — `fleet graph`/`impact` shell -> fleet-context
    lifecycle_cmd.rs                            # 36 — `fleet lifecycle` shell
    meter_cmd.rs                                 # 23 — `fleet meter` shell
    ledger_cmd.rs                                 # 36 — ledger/rollback/pr shells
    verify_cmd.rs                                  # 66 — oracle/adjudicate/attest/gate/contract shells
    verify_cmd_tests.rs                             # 46 — verify_cmd unit tests
    verify_ports.rs                                  # 52 — verify-stage port/adapter wiring
    ops_cmd.rs                                        # 68 — status/doctor/version/completions +
                                                       #      not_yet_implemented() stubs (see note above)
    walk.rs                                            # 71 — bounded graph-walk helper shared by dispatch cmds
  print/
    human.rs                     # 13  — human-formatted line helper
    json.rs                      # 8   — serde_json::to_string_pretty wrapper
    mod.rs                       # 2   — re-exports
  tests/
    support/
      mod.rs                      # 33 — shared test harness
      m4.rs                        # 66 — fixture support
    cli_parses_every_subcommand.rs  # 48 — table-driven: every real subcommand string parses
    pipeline_resumes_after_crash.rs  # 64 — kills the process mid-pipeline, asserts StepLog resume
    gate_and_rollback_exit_codes.rs   # 34 — exit-code mapping regression tests
    graph_bounded_walk.rs              # 42 — dispatch/walk.rs bounds test
    sow_ambiguity_reads_content.rs      # 53 — sow_probes.rs content tests
    agent_child_dispatch.rs              # 56 — __agent child-side dispatch tests
    agent_child_error_cases.rs            # 66 — __agent error-mapping tests
    no_orphaned_command_handlers.rs        # 25 — every Commands variant has a dispatch arm
    no_orphaned_commands_variants.rs        # 17 — every dispatch arm has a Commands variant
```
> If any file above still projects over 80 lines once bodies land, split again. The file-size gate
> (§10) runs before Opus review. `cli/root.rs` and `dispatch/agent_cmd_run.rs` are already at/near
> the 80-line ceiling.

`Cargo.toml` (as built, `src/Cargo.toml`):
```toml
[package]
name = "fleet-cli"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "fleet"
path = "main.rs"

[dependencies]
clap = { version = "4", features = ["derive"] }
clap_complete = "4"
figment = { version = "0.10", features = ["env"] }
tokio = { version = "1", features = ["io-std", "macros", "rt", "rt-multi-thread", "sync", "time"] }
rayon = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
fleet-types = { path = "../crates/fleet-types" }
fleet-lifecycle = { path = "../crates/fleet-lifecycle" }
fleet-store = { path = "../crates/fleet-store" }
fleet-events = { path = "../crates/fleet-events" }
fleet-router = { path = "../crates/fleet-router" }
fleet-scan = { path = "../crates/fleet-scan" }
fleet-plan = { path = "../crates/fleet-plan" }
fleet-context = { path = "../crates/fleet-context" }
fleet-verify = { path = "../crates/fleet-verify" }
fleet-merge = { path = "../crates/fleet-merge" }
fleet-memory = { path = "../crates/fleet-memory" }
fleet-govern = { path = "../crates/fleet-govern" }
fleet-stream = { path = "../crates/fleet-stream" }
fleet-worker = { path = "../crates/fleet-worker" }

[dev-dependencies]
tempfile = "3"
```

## 9. Test plan

**Unit tests:**
- `cap_is_never_zero_even_when_every_input_forces_zero` — `ConcurrencyCap::compute(2, 0, 0)` (or any
  combination forcing the raw `min` to 0) still returns `.get() == 1` (§6's "zero" row).
- `cap_is_the_strict_minimum_of_the_three_inputs` — for a table of `(cores, ram, review)` triples,
  assert `.get()` equals the arithmetic minimum after the cores-minus-2 adjustment, floored at 1.
- `cap_saturates_instead_of_underflowing_on_tiny_core_counts` — `available_cores` of 0, 1, 2 all
  produce a valid (`>=1`) cap, never a panic from unsigned underflow.
- `pipeline_stage_edges_are_exhaustive_and_directional` — every `PipelineStage` variant's allowed
  successor set (from `pipeline/stage.rs`'s edge table) is exactly `{next stage, Teach}`, and `Teach`
  has no successor (terminal).
- `blueprint_q_enqueue_requires_locked_marker` — asserts (via `fleet_lifecycle::Task<Locked>`, a
  compile-time marker, so this is partly a `trybuild` compile-fail test, partly a runtime check for
  the dynamic dispatch path) that `channels.rs`'s enqueue fn has no code path accepting an unlocked task.

**Integration tests (drive the real compiled `fleet` binary, `tests/cli_parses_every_subcommand.rs`):**
- `every_documented_subcommand_parses_without_panicking` — table of every subcommand string
  enumerated in §5's citation row (`meter`, `route`, `roles`, `swarm ...`, `sow`, `plan`, `skills`,
  `role-check`, `agents list`, `lifecycle`, `run`, `oracle`, `adjudicate`, `attest verify`, `pr emit`,
  `status`, `rollback`, `ledger`, `contract`, `gate`, `freeze`, `console`, `graph`, `impact`, `mcp`,
  `completions bash|zsh|fish`, `doctor`, `version`, `help`) — `Cli::try_parse_from` succeeds (or
  fails with a clap usage error for intentionally-invalid arg combos, never panics) for every one.
- `unknown_subcommand_fails_at_parse_not_at_dispatch` — `Cli::try_parse_from(["fleet","bogus"])` is
  `Err` from clap itself, replacing `main.rs:247-250`'s runtime `_ => {...}` fallback.
- `help_and_completions_are_generated_not_hand_written` — `clap::CommandFactory::command()` renders
  help text listing every subcommand; `clap_complete::generate` for each of bash/zsh/fish produces
  non-empty output — regression guard against re-introducing a hand-maintained `print_help`/
  `print_completions` that can drift from the real subcommand list (as `main.rs:4984-5214` risked).

**Mutation-testing targets (`cargo mutants`):**
- Flipping `.max(1)` to a no-op in `ConcurrencyCap::compute` must be killed by
  `cap_is_never_zero_even_when_every_input_forces_zero`.
- Flipping `.min(ram_lanes)`/`.min(review_cap)` to use only one of the three ceilings must be killed
  by `cap_is_the_strict_minimum_of_the_three_inputs`'s table (must include a case where each of the
  three is individually the binding constraint).
- Deleting the "requires Locked" guard in the `blueprint_q`→`build_q` enqueue path must be killed by
  `blueprint_q_enqueue_requires_locked_marker`.

**Property tests:**
- *Cap is monotonic*: for fixed `review_cap`, increasing `ram_lanes` never decreases the computed
  cap (and vice versa) — `proptest`, 200 cases minimum, guards against an accidental `<` vs `<=` or
  min/max swap regression as the function evolves.

## 10. Verification recipe

```bash
cd src   # or the workspace root once fleet/keel/fleet is reshaped to src/ per MIGRATION-PLAN §2
cargo test -p fleet-cli --all-targets
cargo clippy -p fleet-cli --all-targets -- -D warnings
cargo mutants -p fleet-cli
find src tests -name '*.rs' -exec wc -l {} + | awk '$1>80{print; f=1} END{exit f}'  # must print nothing
```
Expected: all unit + integration tests pass, 0 skipped — publish `<passed>/<total>` (e.g. `19/19`),
never just "tests pass". Clippy: 0 warnings. Mutants: every §9 target caught, publish
`<caught>/<total mutants>`. File-size gate: no output (every file ≤80 lines).

## 11. L8 checklist

- [ ] Every fallible path returns a typed error enum (`PipelineError`, each crate's own error type
      forwarded) — no `.unwrap()`/bare `String` error in non-test code; `main.rs` today has several
      `.map_err(|_| EXIT_INVARIANT)` sites (e.g. `main.rs:211`) that collapse a real error into an
      opaque exit code — the refactor must NOT reproduce this pattern for new code even though the
      exit-code boundary at the very top (`process::exit`) is unavoidable.
- [ ] Clock/RNG/IO: this layer is the outermost, so it legitimately touches real IO — but every such
      touch is named in §4/§7, and no *business logic* reads the clock/RNG/env directly (that's each
      crate's job to accept them as parameters).
- [ ] Thread-safety documented: `ConcurrencyCap` is a plain `Copy` value type (`Send + Sync` for
      free); `pipeline::service`'s Restate service is stateless per MIGRATION-PLAN's DAG (each run
      keyed by `TaskId`, no shared mutable state across runs) — say so explicitly in `service.rs`'s
      doc comment when built.
- [ ] No float used for any precision-sensitive count — `ConcurrencyCap`, lane counts, all `usize`.
- [ ] No self-grading — verification runs `cargo mutants` and a real process-kill test
      (`pipeline_resumes_after_crash.rs`), not just unit tests asserting the code compiles.
- [ ] The verify command's pass/fail denominator is published in this file (§10, template) and
      restated in the PR once built.
- [ ] Tests touching the filesystem use `tempfile::TempDir` only, never the repo tree or `$HOME`.
- [ ] Every non-goal in §2 is actually absent from `src/` post-refactor — no `blake3`/hashing import,
      no `libc::dup2`, no tree-sitter parser, no oracle/adjudication logic, anywhere under `src/`;
      enforce with `grep -rn 'blake3::\|dup2\|tree_sitter::' src/` returning nothing.
- [ ] **No source file exceeds 80 lines** (verified: §10's `wc -l | awk` gate). `main.rs` shrinks
      from 6460 lines to ~20; `swarm.rs` (910 lines) ceases to exist under `src/` entirely.

## 12. Definition of Done

`fleet-cli` is DONE when: every OTHER crate in the MIGRATION-PLAN roster is at least `build-done`
(this is explicitly the LAST crate to build, per MIGRATION-PLAN §3's roster and §4 phase P5 —
integration cannot precede the pieces it integrates); §10's four commands all pass with a published
denominator; every §11 box is checked with real numbers; the pipeline runs one real task end-to-end
through all 8 stages against real (not stubbed) crate implementations, including one deliberate
mid-pipeline process kill that proves Restate resumption (§6/§9); `registry/services/REGISTRY.md`
lists `fleet-cli`/`src/` as the composition root; and Opus has re-derived the 8-stage graph and the
`ConcurrencyCap` three-way-min from this blueprint alone, reproduced the "cap never zero" mutation by
hand, and confirmed no business logic (hashing, subprocess spawning, routing decisions, state-machine
transitions) survived under `src/` outside of what §2 explicitly permits.

---

## Divergences from MIGRATION-PLAN (for Opus)

1. **`main.rs` is not "a dispatcher with some logic" — it is ~6200 lines of crate-owned business
   logic wearing a 250-line CLI face.** MIGRATION-PLAN's one-line description ("`main.rs` dispatcher
   + `swarm.rs` → composition root") undersells how much must physically move OUT before `src/` is
   actually just wiring: ledger hashing (fleet-store), agent subprocess/fd-3 supervision
   (fleet-worker), oracle/adjudication (fleet-verify), rollback/PR-emit (fleet-merge), meter
   (fleet-govern), graph/impact (fleet-context), console (fleet-stream), swarm's entire agent
   registry (fleet-worker). Every one of those is flagged with a real `file:line` citation in §5 —
   worth a line in MIGRATION-PLAN §3's last row so whoever reviews `fleet-cli` doesn't assume the
   refactor is mostly mechanical.

2. **`ConcurrencyCap` (this task's `min(cores-2, ram_lanes, review_cap=3)`) has no clean single
   owner in the current 15-crate roster.** `worktree::lane_cap()` (today's only real precedent,
   `worktree.rs:169-174`) is used by `fleet-worker`'s lane execution, but the concurrency CAP concept
   the brief specifies is really a scheduling/composition decision, not a worktree concern. This
   blueprint puts `ConcurrencyCap::compute` in `fleet-cli` (it's a pure wiring parameter, computed
   once at startup) but `fleet-worker` also needs a lane count for its own batching loop, and nothing
   may depend on `src/` (DAG: `src/` is terminal). Two resolutions are possible and Opus should pick
   one: (a) `fleet-cli` computes the cap and PASSES it into every `fleet-worker`/`fleet-govern` call
   that needs it (no new crate, cap flows down as a parameter — this blueprint's assumption), or (b)
   `ConcurrencyCap` moves into `fleet-govern` (which already owns quota/scheduling concerns per
   MIGRATION-PLAN row 11) so both `fleet-cli` and `fleet-worker` import it from there. This blueprint
   assumes (a); flag if Opus prefers (b).

3. **`run_role_lanes_concurrently`'s `thread::scope`-per-batch mechanism (`main.rs:1156-1188`) is the
   thing the new Restate-backed `Dispatch` pipeline stage supersedes**, but MIGRATION-PLAN never
   names this file as superseded — it only appears indirectly under roster row 13 (`fleet-worker`:
   "crew adapters" / row "src/": "composition root"). Worth an explicit line in MIGRATION-PLAN noting
   that `swarm.rs`'s `thread::scope` batching and `main.rs`'s `__lanes_probe` test scaffold are
   replaced, not merely relocated — the batching-with-guaranteed-join-before-next-batch PROPERTY is
   preserved (now via a bounded channel + rayon scope instead of raw `thread::scope`), but the code
   itself does not survive as a second, parallel concurrency mechanism alongside the pipeline.

4. **`clap`/`clap_complete` were already declared dependencies (`Cargo.toml:8-9`) but never actually
   used** — `main.rs` hand-rolls its own arg matching and its own help/completions text instead of
   using the derive macros the dependency exists for. This is itself worth a PRINCIPLES.md-style
   lesson ("a dependency in Cargo.toml is not evidence the tool is being used" — the same shape as
   the owner's "adopting a tool is not the tool working" principle cited in this repo's top-level
   `CLAUDE.md`) — flagging it here rather than silently fixing it in a way that looks like nothing
   changed.

5. **`valid_artifact_id` (`main.rs:2271-2277`) and `state_dir`'s `$FLEET_STATE` validation shape
   look like they could be `fleet-types` invariants** (an ID-shape predicate, same family as
   `TaskId`) rather than CLI-layer code, but MIGRATION-PLAN's `fleet-types` row (row 1) doesn't cite
   `main.rs` at all today. Flagging rather than deciding — this blueprint keeps both in `fleet-cli`
   for now (state-dir resolution is legitimately config/IO; the ID predicate is small enough that
   moving it is low-stakes either way) but a `fleet-types` owner should weigh in.
