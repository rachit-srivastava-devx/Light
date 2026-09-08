# BLUEPRINT — `fleet-worker`

> Precedence if this blueprint conflicts with `MIGRATION-PLAN.md`: the blueprint wins for
> implementation detail; `MIGRATION-PLAN.md` wins for crate boundary/DAG position. A real conflict
> gets a line in MIGRATION-PLAN §7 (teach-back log), not a silent pick.

---

## 1. Header

- **Crate:** `fleet-worker`
- **One-line purpose:** Spawn one CLI-driven agent inside an isolated, hermetically-provisioned
  worktree+sandbox and collect its typed result over the fd-3 receipt channel, treating stdout as
  advisory only.
- **Build branch:** `refactor` (MIGRATION-PLAN §3 row 13) — the pieces exist and are individually
  tested (`worktree.rs`'s isolation primitive, `skills.rs`'s registry resolution, `mcp.rs`'s
  lease-scoped tool server, `agent.rs`'s file-locked scorecard, `main.rs`'s fd-3 dup2/socketpair
  protocol and the Python crew bridge string embedded in `run_model_agent`), but they are scattered
  across a 6460-line `main.rs` god-file plus four smaller files, none of which know about each
  other's crate boundary today. This is a **file split + one new abstraction** (`CliAdapter`), not a
  straight lift — see the divergence note at the end.
- **Imports:** `fleet-types` (`Role`, `TaskId`/`NodeId`/`LaneId`, `Tokens`, `ExitCode`,
  `Receipt`/`Attestation` wire types — this crate constructs a `Receipt` body per completed lane but
  does not append it to the ledger itself, see §2 non-goals).
- **Imported by:** `src/` (composition root — dispatches a task to `spawn`, reads back
  `LaneHandle`'s result to decide next steps); `fleet-router`'s `Decision.selected_adapter`/
  `resolved_model` name which `CliAdapter` and model this crate should drive, but `fleet-router`
  itself does not import `fleet-worker` (the DAG points the other way: `fleet-worker` reads a
  `Decision`, it is not read by one).

## 2. Responsibility & non-goals

**Owns:** the entire lifecycle of one CLI-driven agent lane — creating its isolated worktree,
building its hermetic environment (skills + MCP tool manifest + system prompt, all sourced from the
repo's committed `.fleet/` tree, never the user's `~/.claude`), spawning the CLI subprocess under
`env_clear()` + an explicit allowlist with `HOME`/`XDG_*` redirected to a per-lane tempdir, enforcing
`RLIMIT_AS` + `setpgid` process-group supervision so a runaway lane can be killed as a whole tree,
reading its authoritative result off fd-3 (length-prefixed, blake3-tagged `Receipt`), and reporting a
typed `LaneOutcome` — success, refusal, or environment fault — to the caller. It also owns the
`probe_no_ambient` experiment that answers the single highest-risk open question this crate carries
(see the risk note in §12): whether a CLI can be driven with zero ambient credential leakage at all,
or whether some CLIs silently fall back to `~/.claude`/`~/.codex` state when their expected files are
absent.

**Non-goals (the seam):**
- Does **not** decide which adapter/model to use — that is `fleet-router`'s `decide()`. This crate
  receives an already-resolved `(adapter_kind, requested_model)` pair from the caller and drives
  exactly that; it never re-derives a routing decision or falls back to a different adapter on its
  own initiative.
- Does **not** measure token quota, cooldowns, or write to the meter state file — that is
  `fleet-govern`'s job (today's `route.rs::runtime()`, destined there per MIGRATION-PLAN §3 row 4's
  divergence note). This crate only reports the `Tokens` it observed a completed lane consume; it
  does not decide whether that lane was affordable to start.
- Does **not** append receipts to the hash-chained ledger — that is `fleet-store`'s
  `append_receipt` (today's `crate::append_receipt`, called from `mcp.rs:242` and
  `agent_command`/`run_freelane_agent`/`run_model_agent`'s fd-3 send path in `main.rs`). This crate
  constructs the `Receipt` *body* (the typed fields fd-3 delivered) and returns it to the caller;
  the caller decides when and whether to persist it.
- Does **not** parse CLI arguments, print human/JSON output, or classify user intent — that is
  `src/` (today's `agent_command`/`plan_command` dispatch in `main.rs`) and `fleet-plan`'s
  `intent::classify`.
- Does **not** evaluate the role safety-gate (`LEAD_WROTE_CODE`/`SELF_VERIFIED`) — that already
  lives in `fleet-router` as `evaluate_role_check` (blueprint-done, Opus-approved). This crate
  trusts that the caller only asked it to spawn a role/task combination that already passed that
  gate.
- Does **not** run the fleet-crew Python adapters in-process — `fleet-crew` is a **runtime
  subprocess dependency** (invoked via `python3 -c <bridge>` exactly as `run_model_agent` does
  today), never a compile-time Rust dependency. A missing/broken `crew` environment is reported as a
  typed `LaneOutcome::EnvironmentFault`, never a panic.
- Does **not** touch the user's real `~/.claude`, `~/.codex`, `~/.config`, or any ambient credential
  store under any circumstance — every hermetic env var this crate sets points into a per-lane
  tempdir it created and will remove. If a future change needs to read real user config, that is
  itself a defect in this crate, not a feature to add quietly.

## 3. Public API contract

```rust
//! Spawn one CLI-driven agent inside an isolated worktree + hermetic sandbox, and collect its
//! authoritative result over the fd-3 receipt channel.
//!
//! This crate performs real IO (subprocess spawn, socketpair, filesystem) by design -- unlike
//! `fleet-router`, which is pure. Every IO boundary is named here so a caller can reason about
//! failure modes: worktree creation/removal (`git worktree`, shared with `fleet-merge`'s spawn
//! concern), subprocess spawn + fd-3 recv (this crate's core), and the hermetic tempdir the
//! sandbox lives under. Nothing here reads the user's real `HOME`/`~/.claude`/`~/.codex` -- every
//! credential/config path a spawned CLI sees is either absent (env_clear()) or redirected into a
//! throwaway per-lane directory this crate owns and deletes.

use fleet_types::{ExitCode, LaneId, Receipt, Role, TaskId, Tokens};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Which CLI family a lane drives. Mirrors `main.rs::agent_command`'s match arms
/// (`"claude" | "codex"` today; `"freelane"` is the keyless fallback, `"stub"`/`"stub-verifier"`
/// are test-only and stay test-only here too, gated behind `#[cfg(test)]`-visible construction).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum CliAdapter {
    Claude,
    Codex,
    Freelane,
}

impl CliAdapter {
    /// The executable name this crate checks for on `PATH` before spawning (`main.rs:3584`'s
    /// `which_on_path(cli)` check). `Freelane` has no CLI binary of its own -- it shells out to
    /// the repo-committed `bin/freelane.sh` script instead, so this returns `None` for it.
    pub fn cli_binary_name(self) -> Option<&'static str> { unimplemented!() }
}

/// Everything needed to spawn one lane. The caller (composition root, informed by
/// `fleet-router`'s `Decision`) already resolved which adapter/model to use; this crate does not
/// re-derive that.
#[derive(Clone, Debug)]
pub struct SpawnRequest {
    pub repo: PathBuf,
    pub role: Role,
    pub task_id: TaskId,
    pub adapter: CliAdapter,
    /// `None` for `Freelane` (it has no model parameter) and for `stub`-shaped test lanes.
    pub requested_model: Option<String>,
    /// The free-text task prompt. Empty/whitespace-only is refused (mirrors `main.rs:3577`'s
    /// `run_model_agent` and `main.rs:3351`'s `run_freelane_agent` empty-task checks).
    pub task: String,
    /// Wall-clock budget before this crate escalates to SIGTERM then SIGKILL of the whole process
    /// group. Mirrors `main.rs:3112`'s per-agent deadlines (30s normal, 125s for freelane) --
    /// callers should pass those values; this crate does not hardcode them so a caller can tune
    /// per environment.
    pub deadline: Duration,
}

/// A live, spawned lane: an isolated worktree plus the child process driving it. Dropping this
/// value WITHOUT calling `join` leaks the worktree and the sandbox tempdir -- mirrors
/// `worktree.rs`'s documented non-`Drop` cleanup contract (a silently-swallowed `Drop` failure is
/// exactly the kind of proxy this codebase's standing law rejects). Callers MUST pair every
/// `spawn` with a `join` on both the success and the error path.
pub struct LaneHandle {
    pub lane_id: LaneId,
    pub worktree_path: PathBuf,
}

/// Why a spawn attempt never reached the point of producing a `LaneOutcome`.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SpawnError {
    #[error("task is empty or all-whitespace")]
    EmptyTask,
    #[error("adapter {0:?} requires the `{1}` CLI on PATH, which is not installed")]
    CliNotOnPath(CliAdapter, &'static str),
    #[error("worktree creation failed after retries (git worktree add exit fault)")]
    WorktreeCreateFailed,
    #[error("hermetic sandbox provisioning failed: {0}")]
    SandboxProvisionFailed(String),
    #[error("socketpair() failed to establish the fd-3 channel")]
    Fd3ChannelUnavailable,
    #[error("subprocess spawn failed: {0}")]
    ProcessSpawnFailed(String),
}

/// The authoritative outcome of one lane, read off fd-3. Stdout/stderr are NEVER consulted to
/// determine this value -- fd-3 is the only authoritative channel (this crate's central invariant,
/// mirroring `main.rs:3125-3154`'s `spawn_agent_with_args`/`agent_command` design, which already
/// discards stdout/stderr for the model-agent path and treats an empty or malformed fd-3 packet as
/// `EXIT_INVARIANT`, never as success).
#[derive(Clone, Debug)]
pub enum LaneOutcome {
    /// The CLI completed and reported `kind: "done"` on fd-3, `validate_submission`-clean.
    Done {
        resolved_model: Option<String>,
        tokens: Option<Tokens>,
        body: serde_json::Value,
    },
    /// The CLI reported `kind: "refuse"` on fd-3 -- an expected, typed non-success, not an error
    /// (mirrors `main.rs`'s `send_agent_packet("refuse", ...)` call sites).
    Refused { reason: String },
    /// fd-3 delivered nothing, a malformed packet, or the process never produced a result before
    /// the deadline fired and it was killed. Mirrors `main.rs:3141-3152`'s empty/invalid-packet
    /// handling and `wait_with_deadline`'s timeout path -- both are `EXIT_INVARIANT` today, never
    /// silently treated as `Refused` (a timeout is not a considered refusal).
    EnvironmentFault { detail: String },
}

/// Join a spawned lane: wait for the process, drain fd-3 (or the deadline), tear down the
/// worktree+sandbox regardless of outcome, and return the result. Removal failure is itself
/// reported, not swallowed -- mirrors `worktree.rs::remove`'s "removed is a claim, not an
/// assumption" contract.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum JoinError {
    #[error("worktree/sandbox teardown failed after the lane finished: {0}")]
    TeardownFailed(String),
}

/// Spawn one lane. Creates the isolated worktree, provisions the hermetic sandbox (skills + MCP
/// manifest + system prompt from the repo's committed `.fleet/` tree only), spawns the CLI
/// subprocess with `env_clear()` + allowlist and `HOME`/`XDG_*` redirected into a fresh per-lane
/// tempdir, sets up the fd-3 socketpair, `dup2`s the child's end onto fd 3, and returns a
/// `LaneHandle` immediately (non-blocking past process spawn) -- the caller calls `join` to
/// collect the result.
pub fn spawn(request: SpawnRequest) -> Result<LaneHandle, SpawnError> { unimplemented!() }

/// Block until the lane's process exits or `request.deadline` elapses (escalating SIGTERM then
/// SIGKILL to the whole process group on timeout, mirroring `main.rs::wait_with_deadline`/
/// `terminate_group`), drain and validate the fd-3 packet, and tear down the worktree + sandbox
/// tempdir unconditionally (success or failure) before returning.
pub fn join(handle: LaneHandle) -> Result<LaneOutcome, JoinError> { unimplemented!() }

/// The M1 keyless-thesis experiment: attempt to drive `adapter` with EVERY ambient credential
/// source removed (`env_clear()`, `HOME`/`XDG_*` pointed at an empty tempdir with no pre-seeded
/// auth files) and report whether it still produced a usable result, refused cleanly, or -- the
/// dangerous case this probe exists to catch -- silently fell back to some ambient state this
/// crate did not intend to expose (e.g. a CLI that caches credentials outside `HOME`). This is
/// NOT the same question as `LaneOutcome::Refused` -- a clean refusal is a PASS for this probe (it
/// proves the CLI correctly detected it had no credentials); only an unexplained `Done` or a leak
/// of real ambient state is a FAIL. See §12 "highest-risk unknown" -- this fn's result is the
/// single most important thing this crate reports back to Opus.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NoAmbientProbeResult {
    /// The CLI correctly refused or failed for lack of credentials -- the keyless-hermetic
    /// contract holds.
    CleanlyBlocked { detail: String },
    /// The CLI produced a `Done` result with no ambient credentials visible to it -- either it is
    /// genuinely keyless-capable (e.g. `freelane`) or it found credentials this crate failed to
    /// block, which this probe cannot distinguish alone; the caller (a human, per §12) must
    /// inspect `evidence_paths` by hand.
    UnexpectedSuccess {
        detail: String,
        evidence_paths: Vec<PathBuf>,
    },
}
pub fn probe_no_ambient(adapter: CliAdapter, repo: &Path) -> Result<NoAmbientProbeResult, SpawnError> {
    unimplemented!()
}

/// The committed skill registry resolved against one agent's declared capabilities --
/// hermetic-provisioning seed data this crate feeds into the sandbox before spawn. Wraps
/// `skills.rs`'s `Registry`/`Agent` resolution (today loaded from repo-root `skills.toml`/
/// `agents.toml`; this crate resolves the SAME shape from the repo's `.fleet/` tree instead, see
/// §5).
pub struct HermeticProvision {
    pub skill_ids: BTreeSet<String>,
    pub mcp_tool_manifest: serde_json::Value,
    pub system_prompt: String,
}

/// Why hermetic provisioning could not assemble a `HermeticProvision` for one agent/lease.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ProvisionError {
    #[error("repo .fleet/ tree is missing required file: {0}")]
    MissingFleetFile(&'static str),
    #[error("skill {0:?} is declared by the agent but absent from the committed registry")]
    UnresolvedSkill(String),
    #[error("agent {0:?} declares skill {1:?} but is missing required capabilities: {2:?}")]
    MissingCapabilities(String, String, Vec<String>),
}

/// Resolve one agent's hermetic provision from the repo's committed `.fleet/` tree. Never reads
/// `$HOME` or any path outside `repo` -- every input is repo-committed and version-controlled
/// (PLAYBOOK.md rule 8, "institutional knowledge is code").
pub fn resolve_hermetic_provision(
    repo: &Path,
    agent_id: &str,
) -> Result<HermeticProvision, ProvisionError> {
    unimplemented!()
}
```

## 4. Data model & invariants

| Type | Invariant enforced | Illegal state made unrepresentable |
|---|---|---|
| `SpawnRequest.task` | Non-empty after `.trim()`, checked at the top of `spawn` before any IO happens. | A worktree/sandbox being created and then torn down immediately for a request that was always going to be refused — mirrors `run_model_agent`/`run_freelane_agent`'s early empty-task check, moved before IO instead of after CLI-presence check as today (a cheap ordering fix, not a behavior change). |
| `LaneOutcome` | Exactly one of `Done`/`Refused`/`EnvironmentFault` — a Rust enum, not three separate `Option`s that could all be `Some` or all be `None`. | A caller observing `resolved_model.is_some()` alongside a live refusal, which the current `Option<Value>` return type in `main.rs::spawn_agent_with_args` cannot prevent at the type level. |
| `LaneHandle` | Not `Clone`, not `Copy` — one `join` call consumes it exactly once. | Calling `join` twice on the same lane (double-wait on an already-reaped child, or double-removing an already-removed worktree). |
| `HermeticProvision` | Constructed only via `resolve_hermetic_provision`, which reads exclusively from `repo`-relative paths under `.fleet/`. | A sandbox silently inheriting a skill/MCP tool/system-prompt fact from the user's real `~/.claude` because some code path fell back to an ambient default. |
| `NoAmbientProbeResult::UnexpectedSuccess.evidence_paths` | Always non-empty when this variant is constructed — the probe must name *where* it suspects a leak, never just assert "something's wrong." | An unexplained probe failure with no actionable evidence, which is exactly the "adopting a tool is not the tool working" failure mode PRINCIPLES.md warns about. |
| Process-group supervision (`spawn`'s subprocess) | Every spawned child is placed in its own process group (`setsid`, mirroring `main.rs:2953-2963`'s `start_process_group` and `3083-3096`'s `pre_exec`) and every timeout kill targets `-pid` (the whole group, mirroring `terminate_group`/`libc::kill(-(child.id()...` at `main.rs:3174/3190`), never the single child pid. | A killed CLI leaving orphaned grandchild processes (e.g. the Python bridge's own subprocess) running past the lane's declared deadline. |

**Money/precision:** no money type in this crate. `Tokens` (from `fleet-types`) is the only
count-like value crossing this crate's boundary, and it is `fleet-types`'s integer-minor-unit
newtype — this crate never re-derives a token count itself, only parses what fd-3/the crew bridge
reported (mirrors `main.rs::parse_freelane_usage`'s `prompt + completion == total` cross-check,
which this crate's fd-3 parsing must preserve, not weaken, when porting).

**Clock/RNG/IO injection points:** this crate is IO-heavy by design (§2's "Owns" is entirely IO) —
every boundary is named explicitly, not hidden:
- **Subprocess spawn** (`spawn`/`join`): direct — `std::process::Command`, not caller-injected,
  because the whole point of this crate is to own this boundary; a future test seam should wrap it
  behind a trait (`ProcessSpawner`) if `fleet-worker`'s own tests need to fake a CLI without a real
  binary — flag this as an open question for Opus (see §12), since `fleet-router`'s exemplar has no
  equivalent because it has zero IO.
- **Filesystem** (worktree create/remove, sandbox tempdir, `.fleet/` reads): direct, via `std::fs`
  and `git` subprocess calls — mirrors `worktree.rs` verbatim (see §5); no injection today, same
  open question as above.
- **Clock**: `join`'s deadline uses `std::time::Instant::now()` internally (mirrors
  `main.rs::wait_with_deadline`) — not caller-injected. A test that needs deterministic timeout
  behavior should use a very short `Duration` against a real slow/blocked child rather than faking
  time; this crate does not introduce a clock trait fleet doesn't already have.
- **RNG**: the retry backoff in worktree creation uses the same dependency-free jitter as
  `worktree.rs:69-76` (PID xor timestamp xor attempt number) — not a real RNG, and this crate keeps
  it that way rather than pulling in a `rand` dependency for a jitter that only needs to
  desynchronize, not be unpredictable.

## 5. Reuse map

Source files read in full 2026-09-08: `fleet/keel/fleet/src/worktree.rs` (243 lines),
`fleet/keel/fleet/src/skills.rs` (304 lines), `fleet/keel/fleet/src/agent.rs` (476 lines),
`fleet/keel/fleet/src/mcp.rs` (537 lines), and the fd-3/spawn section of `fleet/keel/fleet/src/main.rs`
(read lines 2930-3679 of 6460 total).

| Fleet source (file:line) | What it does today | Lift as-is? | Change needed |
|---|---|---|---|
| `worktree.rs:1-46` (`EXIT_ENV`/`EXIT_INVARIANT` consts, `Worktree` struct, `unique_name`) | Named-branch worktree primitive, pid+counter naming to avoid collisions. | yes | `pub` already; replace local `i32` exit codes with `fleet_types::ExitCode`/`SpawnError` variants at this crate's boundary. **This module is explicitly shared with `fleet-merge`'s spawn concern** (MIGRATION-PLAN row 9) — this blueprint takes only the *spawn* half (`create`/`unique_name`/`lane_cap`); `remove`'s teardown-on-join half is also needed here (see next row), so the actual split is: `fleet-worker` owns create+remove for lanes IT spawns, `fleet-merge` owns the same primitive for ITS OWN worktree needs (merge staging) — they do not share a single `Worktree` instance, they share the *module*, likely via a tiny shared internal crate or by each vendoring the ~230-line primitive. **Flagging for Opus**: MIGRATION-PLAN doesn't currently name which crate owns the canonical copy — see divergence note. |
| `worktree.rs:48-116` (`create`) | `git worktree add -b fleet/<name>`, retried up to 8x with jitter backoff on transient lock contention, pruning between attempts. | yes | Port verbatim into `spawn`'s worktree step; the retry-on-lock behavior is load-bearing (documented as observed, not hypothetical, at `worktree.rs:58-64`) and must not be "simplified" away. |
| `worktree.rs:118-164` (`remove`) | `git worktree remove --force` + filesystem fallback + best-effort branch/prune cleanup, errors collected not swallowed. | yes | Port verbatim into `join`'s teardown step; `JoinError::TeardownFailed` surfaces what `remove` today returns as `Err(i32)`. |
| `worktree.rs:166-174` (`lane_cap`) | `min(16, available_parallelism()-2)`, floored at 1. | yes | Not part of this crate's public API directly (it's a scheduling concern), but this crate's `spawn` must respect a caller-supplied concurrency cap rather than reimplementing `lane_cap` itself — `fleet-govern`/`src/` calls `lane_cap`-equivalent logic and rate-limits calls to `spawn`. Flag: which crate owns `lane_cap` itself is another DAG question for Opus (candidates: `fleet-govern`, or it moves to `fleet-types` as a pure fact). |
| `skills.rs:1-194` (`SkillFile`/`RegistryFile`/`Skill`/`Registry`/`Resolution*`/`resolve`/`evaluate`/`require_for_agent`) | TOML-backed skill registry, resolved against an `AgentRegistry`'s declared capabilities; `Registry::load_default` reads repo-root `skills.toml`. | **partial** | The resolution *logic* (`resolve`/`evaluate`/`require_for_agent`, `ResolutionStatus`/`ResolutionReport`) ports near-verbatim into `resolve_hermetic_provision`'s implementation. The *loading* changes: today's `load_default` reads `skills.toml`/`agents.toml` at repo root; this crate's hermetic seed must read from a repo-committed `.fleet/` tree instead (per the task brief's HERMETIC requirement) — **this is new structure, not present in fleet today**; flag for Opus whether `.fleet/skills.toml` replaces the root file or the root file itself is what "hermetic" means (i.e. maybe no new tree is needed, just a stricter read path). |
| `skills.rs:196-242` (`command`) | CLI-facing `fleet skills [--check]` printer. | no | Presentation/CLI — `src/`'s job, not this crate's. |
| `skills.rs:244-304` (`#[cfg(test)] mod tests`) | 3 tests: unresolved-skill caught, fully-resolved passes, empty-registry fails. | as inspiration | Port the assertions into this crate's `resolve_hermetic_provision` test plan (§9), adapted to the `.fleet/`-tree loading path. |
| `agent.rs:1-125` (`AgentFile`/`Agent`/`RegistryFile`/`Registry`, `assignment`) | TOML-backed agent registry + `Agent` accessor + `Registry::assignment` builder for the `Assignment<Idle>` state machine. | **no, out of scope** | This is agent-*identity* data (capabilities/skills declared per agent), consumed by `skills.rs`'s resolution — needed as an input to `resolve_hermetic_provision`, but the `Agent`/`Registry` types themselves belong wherever `fleet-lifecycle`/`fleet-types` puts agent identity (not decided in the roster yet); this crate takes an `agent_id: &str` and does its own minimal lookup rather than depending on the full `Assignment<S>` state machine. Flag for Opus: is there an `AgentRegistry` home crate, or does `fleet-worker` own a trimmed copy? |
| `agent.rs:127-289` (`Scorecard`, `scorecard_path`, `load_scorecard`, `persist_scorecard`, `record_scorecard_outcome`) | The file-locked (via `fs2::FileExt::lock_exclusive`) read-modify-write scorecard, fixing D54b's lost-update race under concurrent dispatch. | **yes — re-homes here per MIGRATION-PLAN §7/task brief** | Port verbatim: the `fs::OpenOptions::new().create(true).write(true)` lock-file pattern, the exclusive lock held across the whole read-modify-write closure, and the `let _ = FileExt::unlock(&lock);` unconditional release are all load-bearing (this is the exact fix for a real race the file's own comment documents). This crate calls `record_scorecard_outcome` once per completed lane, keyed on the lane's `agent_id` and the `LaneOutcome` it produced (`Done`→`Credited`, explicit `Fault` only on a caller's later adjudication — `record_scorecard_outcome` itself does not decide credit/fault, `join`'s caller does, mirroring today's separation between `Scorecard::record_*` and `Verdict`). |
| `agent.rs:291-476` (`list`, `Idle..Amended` marker structs, `Assignment<S>` type-state machine, `Verdict`, tests) | CLI printer + the compile-time-enforced assignment lifecycle (`assign`→`brief`→`work`→`submit`→`adjudicate`→`credit`/`fault`→`amend`). | **no** | This is `fleet-lifecycle`'s concern per MIGRATION-PLAN's 15th-crate addition (the type-state `Task<S>` machine) — `Assignment<S>` here is a parallel, smaller instance of the same pattern applied to agents rather than tasks. Flag for Opus: should `Assignment<S>` unify with `fleet-lifecycle`'s `Task<S>`, or is it a legitimately separate state machine this crate keeps? This blueprint does NOT lift it — `fleet-worker` only calls `record_scorecard_outcome`, it does not manage the `Idle→Amended` transitions. |
| `mcp.rs:1-146` (`FileReadParams`.. `ManifestTool`/`ToolManifest`/`Lease::parse`/`Lease::manifest`) | Lease-scoped MCP tool manifest (`file_read`/`file_write`/`file_list` scoped to a `<prefix>/**` lease, plus always-on `impact`/`lesson_recall`/`ledger_read`). | yes | This is exactly the seed for `HermeticProvision.mcp_tool_manifest` — a lane's MCP tool set is the lease-scoped manifest for that lane's own worktree prefix. Port `Lease`/`ToolManifest`/`manifest_for_lease` near-verbatim; `manifest_for_lease`'s signature (`&str -> Result<Value, i32>`) becomes `resolve_hermetic_provision`'s internal call, replacing `i32` with `ProvisionError`. |
| `mcp.rs:148-259` (`serve`, `LeaseServer::new`/`routes`/`route`/`allows`/`outcome`) | Actually runs the rmcp stdio server, dispatching to handlers and writing a receipt (`crate::append_receipt`) per tool call. | **no — this is the MCP SERVER, not this crate's concern** | `fleet-worker` only needs the *manifest* (what tools a lane may see) to hand to the spawned CLI at provisioning time — running an actual MCP server process is either the spawned CLI's own MCP client talking to a separate `fleet mcp serve` process (today's `mcp.rs::serve`, likely staying in `src/`/a future `fleet-events`-adjacent home) or, if this crate spawns the MCP server itself as a sidecar per lane, that is additional scope not in the task brief — flag for Opus which shape is intended; this blueprint assumes the MANIFEST is hermetic sandbox input and the SERVER is out of scope. |
| `mcp.rs:261-470` (`authorized_path`, `file_read`/`file_write`/`file_list`/`impact`/`lesson_recall`/`ledger_read` handlers, `normalize_relative`) | Path-traversal-safe lease enforcement + the tool bodies themselves. | no | Tool *execution* is the MCP server's job (see above), not the sandbox-provisioning crate's. `normalize_relative`'s traversal-rejection logic is worth citing as a pattern this crate's own `.fleet/`-tree path resolution (`resolve_hermetic_provision`) should reuse defensively, even though it isn't lifted verbatim. |
| `mcp.rs:509-537` (tests) | 2 tests: manifest-from-lease shape, traversal rejection. | as inspiration | Port the traversal-rejection assertion into this crate's `.fleet/`-tree-read tests if `resolve_hermetic_provision` accepts any caller-influenced path component (agent_id) — it should reject `agent_id` values containing `/`or `..` defensively, same failure mode. |
| `main.rs:2943-2963` (`run_bounded`, `start_process_group`) | Generic bounded-subprocess runner + `setsid` process-group setup via `pre_exec`. | yes | Port `start_process_group`'s `pre_exec`/`setsid` pattern verbatim into `spawn`'s child setup — this is the exact primitive `SpawnRequest`'s process-group supervision requires. |
| `main.rs:3008-3155` (`spawn_agent`, `spawn_verifier`, `spawn_agent_with_args`) | The fd-3 protocol core: `socketpair(AF_UNIX, SOCK_SEQPACKET)` with a `SOCK_STREAM` Darwin fallback, `env_clear()` + allowlist (`PATH`/`HOME`/`LANG`/`CARGO_TARGET_DIR`), `pre_exec` doing `setsid`+`dup2(child_fd,3)`, `wait_with_deadline`, `recv` up to 65536 bytes, `validate_submission`. | yes, this is the crate's core | Port verbatim as `spawn`/`join`'s implementation, replacing the bare `i32` exit codes with `SpawnError`/`LaneOutcome`/`JoinError`. **Change needed, flagged as a real gap**: today's allowlist (`PATH`/`HOME`/`LANG`/`CARGO_TARGET_DIR`) is NOT what "HERMETIC... HOME/XDG overridden to a per-lane tempdir" requires — the task brief requires `HOME` to be REDIRECTED, not passed through. This blueprint's `spawn` must NOT reuse `env::var("HOME").ok()` verbatim; it must compute a fresh per-lane tempdir path and set `HOME`/`XDG_CONFIG_HOME`/`XDG_DATA_HOME`/`XDG_CACHE_HOME` to point there instead. This is the single most load-bearing divergence from a straight port — see §12. |
| `main.rs:3157-3192` (`wait_with_deadline`, `terminate_group`) | Poll-with-deadline, escalating SIGTERM→(5s grace)→SIGKILL against the whole process group (`-pid`). | yes | Port verbatim into `join`. |
| `main.rs:3194-3245` (`agent_command` dispatch incl. `"stub"`/`"stub-verifier"`/`"env-probe"` arms, `send_agent_packet`) | CLI-side dispatch for the child process (this runs INSIDE the spawned child, not the parent). | **no, wrong side of the fork** | This code runs as `fleet __agent <kind> ...` — the CHILD's own entrypoint, invoked via `env::current_exe()` at `main.rs:3067`. `fleet-worker` is the PARENT-side crate (spawns and joins); the child-side dispatch (what actually runs claude/codex/freelane and sends the fd-3 packet) stays in `src/`'s binary as the `__agent` subcommand, since only the composition root binary can `dup2`+`exec` itself. This crate's `spawn` constructs the `Command` that invokes `<current_exe> __agent <kind> ...`, but does not itself implement the child-side handlers. Flag for Opus: confirm this parent/child split is the intended crate boundary — it is implied by the brief's `main.rs ~L3088 dup2` citation but not stated explicitly. |
| `main.rs:3247-3348` (`FreelaneOutput`/`FreelaneFailure`, `parse_freelane_model`, `parse_freelane_usage`, `interpret_freelane_output`) | Parses freelane's stderr log lines for `[resolved_model=...]`/`[...usage=...]` markers, cross-checking `prompt+completion==total`. | yes, as pure parsing helpers | These are pure string→struct parsers with no IO — port verbatim as private helpers backing the `Freelane` arm of `spawn`/`join`'s result assembly (they run child-side today; if the child-side `__agent freelane` handler stays in `src/` per the row above, these parsers may need to move there instead — flag for Opus: `fleet-worker` needs the *shape* (`FreelaneOutput`) to interpret what fd-3 for a freelane lane delivers, even if the parsing itself executes in the child). |
| `main.rs:3350-3453` (`run_freelane_agent`) | Child-side: shells to `bin/freelane.sh`, extracts fenced code from the reply (refusing prose-only replies per the D47 fix), sends the fd-3 packet. | **no, child-side** (same split as above) | Cited for the D47 lesson this crate's own tests must regression-guard if any of this logic is duplicated in `fleet-worker`: "a non-empty diff is a proxy; a diff that implements something is the property" — if `fleet-worker` ever gains a role in interpreting `Done` bodies beyond passthrough, it must not regress this fix. |
| `main.rs:3576-3663` (`run_model_agent`, the embedded Python bridge string) | Child-side: checks `which_on_path(cli)`, embeds a Python bridge script that imports `crew.adapters.{ClaudeAdapter,CodexAdapter}`, invokes it with `stdout_path`/`stderr_path`/`diff_path` under a `tempfile.TemporaryDirectory`, and sends `done`/`refuse` on fd-3. | **partial — the CLI-presence check ports, the bridge string is evidence for `fleet-crew`'s contract** | `which_on_path`/the `CliAdapter::cli_binary_name` presence check ports into `spawn` as a pre-flight (fail fast with `SpawnError::CliNotOnPath` before any worktree/socketpair IO, cheaper than today's order which spawns first). The embedded Python bridge itself is `fleet-crew`'s contract surface (MIGRATION-PLAN row 14) — this crate's parent-side `spawn` needs to invoke an equivalent bridge (or `fleet-crew` exposes a stable CLI entrypoint this crate calls instead of embedding a bridge string) — flag for Opus: should the bridge string live in `fleet-worker` (as today, duplicated per language boundary) or does `fleet-crew` publish `python3 -m crew.bridge <agent> <repo> <task> <model>` as its OWN stable interface so `fleet-worker` never embeds Python source as a string literal? This blueprint recommends the latter (a named `fleet-crew` entrypoint) as cleaner but flags it as a change, not a given. |
| `main.rs:3665-3678` (`validate_submission`) | Checks `schema_version=="1.0"`, `kind` in `{note,done,refuse}`, `body` is an object. | yes | Port verbatim into `join`'s fd-3 packet validation, replacing `i32` with a typed variant of `JoinError`/folding into `LaneOutcome::EnvironmentFault` on failure (an invalid packet is an environment fault, not a `Refused` — mirrors today's `EXIT_INVARIANT` classification). |
| `crew/crew/adapters/base.py:1-90` (`AdapterError`, `UsageRecord`, `InvocationResult`, `Adapter` protocol) | The Python-side adapter contract `run_model_agent`'s embedded bridge drives — integer-only usage, file-backed stdout/stderr/diff paths, `exit_code`/`ok` properties. | n/a (Python, not lifted into Rust) | Cited as the CONTRACT this crate's `spawn`/`join` must remain compatible with when invoking `fleet-crew` — `UsageRecord`'s integer-only invariant is exactly why this crate's `Tokens` parsing must reject any non-integer/negative usage value rather than coercing it. |

## 6. Behavior spec

### `fn spawn(request: SpawnRequest) -> Result<LaneHandle, SpawnError>`

| Input dimension | Behavior |
|---|---|
| empty | `request.task.trim().is_empty()` → `Err(SpawnError::EmptyTask)` before any IO (worktree/socketpair/subprocess) is attempted — cheaper-fail than today's order, which spawns Python before checking task emptiness in some paths. |
| null / `None` | `requested_model: None` on `CliAdapter::Claude`/`Codex` is valid (the underlying CLI picks its own default) — not an error; mirrors `main.rs:3233`'s `.filter(|s| !s.is_empty())`. `requested_model: Some(_)` on `CliAdapter::Freelane` is accepted but ignored (freelane has no model parameter) — documented, not an error, since a caller building requests generically for all three adapters should not need adapter-specific field-presence logic. |
| wrong-type | Not reachable inside this crate — `CliAdapter`/`Role` are enums the caller already constructed correctly (same non-goal as `fleet-router`'s stringly-typed-input exclusion); any string→enum parsing happens in `fleet-types`/`src` before `spawn` is called. |
| huge | A `task` string of ~1MB: no length cap in this crate (mirrors today — `main.rs` has none either); passed through to the CLI/crew bridge as-is. `deadline` of `Duration::MAX` is accepted but not recommended — this crate does not clamp it, since clamping is a policy choice `fleet-govern` should own, not silently impose here. |
| negative | n/a — `Duration` cannot be negative; no other numeric input to `spawn`. |
| duplicate | Two concurrent `spawn` calls for the identical `task_id` are NOT rejected by this crate (task-identity uniqueness is a caller/`fleet-lifecycle` concern) — but `worktree::unique_name`'s pid+counter mixing (ported verbatim) guarantees their underlying worktree paths never collide even if `task_id` is identical, exactly as `worktree.rs`'s own `two_concurrent_names_never_collide` test proves today. |
| concurrent | Multiple `spawn` calls from multiple threads: each gets an independently-named worktree (see above) and an independent socketpair/child process; no shared mutable state between concurrent `spawn` calls except the underlying `git worktree add` lock contention already handled by the retry-with-jitter in `create` (ported from `worktree.rs:58-102`, empirically observed at 3-of-4 succeeding immediately under real concurrent load). |
| unicode / non-ASCII | `task`/`requested_model` containing non-ASCII text: passed through byte-for-byte to the subprocess argv / Python bridge argv — no normalization, no rejection; mirrors today's behavior (fleet already accepts arbitrary UTF-8 task text). |
| already-exists | A worktree name collision (extremely unlikely given `unique_name`'s pid+counter, but possible if two separate OS processes both reset their counters) surfaces as `SpawnError::WorktreeCreateFailed` after the 8-attempt retry-with-prune exhausts, exactly as `worktree.rs::create` behaves today — never silently reuses or overwrites an existing worktree directory. |
| partial-failure | Worktree created successfully but hermetic sandbox provisioning fails (`ProvisionError`): `spawn` must remove the just-created worktree before returning `Err(SpawnError::SandboxProvisionFailed(..))` — a `spawn` that returns `Err` MUST NOT leave a worktree behind for the caller to leak (unlike `LaneHandle`'s post-success contract, where cleanup is `join`'s job); this is a genuine new invariant this crate adds beyond a pure port, since today's `worktree.rs`/`main.rs` split means no single fn currently owns "create then immediately fail" cleanup end-to-end. |

### `fn join(handle: LaneHandle) -> Result<LaneOutcome, JoinError>`

| Input dimension | Behavior |
|---|---|
| empty | An fd-3 packet of zero bytes received (`recv` returns `0`) → `LaneOutcome::EnvironmentFault` with a detail naming "agent exited without an fd-3 result," mirroring `main.rs:3141-3146`'s message verbatim (the message text itself is cited as load-bearing operator-facing guidance, not decoration). |
| null / `None` | n/a at this fn's boundary — `handle` is an owned, non-`Option` value; there is no "missing handle" state to handle (the type system already excludes it). |
| wrong-type | An fd-3 packet that parses as JSON but fails `validate_submission` (wrong `schema_version`, unknown `kind`, non-object `body`) → `LaneOutcome::EnvironmentFault`, never silently coerced into `Done`/`Refused` — mirrors today's `EXIT_INVARIANT` on `validate_submission` failure. |
| huge | An fd-3 packet larger than the 65536-byte buffer: `libc::recv` truncates silently at the syscall level today (`main.rs:3125` allocates exactly `65536`) — this crate must NOT regress this into an undetected truncation; **flag for Opus**: this is a real, currently-unaddressed limit in the ported code (a legitimate `Done` body over 64KB — e.g. a large diff — would be silently truncated and then fail `validate_submission`/JSON parse, misreported as `EnvironmentFault` rather than "packet too large"). This blueprint recommends adding a distinct `JoinError`/`LaneOutcome` variant for "packet at the buffer ceiling" (detect `received == packet.len()` and treat it as suspect, per PRINCIPLES.md's "a proxy is not the property" — full-buffer is not proof of exactly-fits) rather than silently porting the ambiguity forward. |
| negative | n/a — no negative-representable input to `join`. |
| duplicate | Calling `join` twice on a `LaneHandle` is prevented by the type system (`join` takes `handle` by value, consuming it) — not a runtime check, a compile error. |
| concurrent | `join` blocks the calling thread until the deadline or process exit — a caller wanting concurrent lanes must call `spawn`+`join` from independent tasks/threads (this crate's `join` itself is not `async`; whether to offer an async variant is flagged for Opus given `tokio` is a listed dependency — see §7). |
| unicode / non-ASCII | fd-3 body containing non-ASCII text (e.g. a diff with unicode identifiers): passed through as `serde_json::Value` unchanged — no normalization, mirrors `LaneOutcome::Done.body`'s pass-through design. |
| already-exists | n/a — `join` does not create anything; it only tears down what `spawn` created. |
| partial-failure | Process exits and fd-3 delivers a valid `Done` packet, but worktree/sandbox teardown then fails (`git worktree remove` fails AND the filesystem fallback also fails) → `Err(JoinError::TeardownFailed(..))` — **the `LaneOutcome` the child produced is LOST in this path** (the fn returns `Result<LaneOutcome, JoinError>`, not both); flag for Opus whether this is acceptable or `join` should return `(Option<LaneOutcome>, Result<(), JoinError>)`-shaped so a valid result is never discarded just because cleanup afterward failed — this blueprint's current signature has that gap, named explicitly rather than hidden. |

### `fn probe_no_ambient(adapter: CliAdapter, repo: &Path) -> Result<NoAmbientProbeResult, SpawnError>`

| Input dimension | Behavior |
|---|---|
| empty | `repo` pointing at a directory with no `.fleet/` tree: the probe still runs (it does not require hermetic provisioning to succeed — the whole point is testing what the CLI does with NOTHING) but `resolve_hermetic_provision` calls inside it, if any, would themselves fail with `ProvisionError::MissingFleetFile` — the probe's own result reports this as part of `detail`, not as a hard `Err`, since "no hermetic tree present" is itself a valid thing to probe. |
| null / `None` | n/a — no optional input. |
| wrong-type | n/a — `CliAdapter` is an enum. |
| huge | n/a — this probe spawns exactly one lane; no collection-sized input. |
| negative | n/a — no numeric input. |
| duplicate | Running the probe twice in sequence for the same adapter is idempotent in intent (each run gets its own fresh per-lane tempdir via the same `unique_name` mechanism as `spawn`) but is NOT guaranteed to produce the identical `NoAmbientProbeResult` — a real CLI's behavior under no-ambient-credentials may itself be non-deterministic (network-dependent) — this crate does not claim determinism here, unlike `fleet-router`'s `decide()`. |
| concurrent | Running probes for two different adapters concurrently is safe (independent tempdirs/worktrees, same guarantee as `spawn`); running the SAME adapter's probe concurrently with itself is safe but wasteful (no shared state to corrupt). |
| unicode / non-ASCII | n/a — no text input beyond `repo`'s path, which follows normal OS path-encoding rules. |
| already-exists | n/a — creates its own fresh tempdir/worktree each call via `unique_name`. |
| partial-failure | If the underlying `spawn`/`join` sequence itself fails (`SpawnError`) before ever reaching a CLI-behavior observation, `probe_no_ambient` propagates that `SpawnError` rather than reporting it as `CleanlyBlocked` — a probe infrastructure failure is NOT evidence about the CLI's credential behavior and must not be conflated with one. |

## 7. Dependencies

> **As built** (`crates/fleet-worker/Cargo.toml`) — several deps proposed below were dropped and
> one (`fleet-merge`) was added; see the note after the table.

| Crate | Version | Why |
|---|---|---|
| `fleet-types` | workspace-path (`{ path = "../fleet-types" }`) | `Role`, `TaskId`/`NodeId`/`LaneId`, `Tokens`, `ExitCode`, `Receipt`/`Attestation` wire types. |
| `fleet-merge` | workspace-path (`{ path = "../fleet-merge" }`) | The worktree primitive (`create`/`remove`/`unique_name`) turned out to be a genuine sibling dependency rather than two independent vendored copies (§5/divergence note 2 flagged this as an open Opus call): `spawn`/`join` call `fleet_merge::create`/`fleet_merge::remove`/`fleet_merge::unique_name` directly (see `src/spawn/mod.rs`, `src/spawn/join_impl.rs`), so `fleet-worker` does not carry its own `worktree.rs` port. |
| `libc` | `0.2.189` | `socketpair`, `dup2`, `setsid`, `recv`, `send`, `kill` — the exact fd-3 primitives `main.rs` already used; no higher-level wrapper for syscalls this codebase had already hand-verified (the Darwin `SOCK_SEQPACKET`→`SOCK_STREAM` fallback is a real, tested platform difference, not decoration). |
| `nix` | `0.29` (features `process`, `resource`, `signal`) | Safer, typed wrappers for `RLIMIT_AS`/`setpgid`/signal-based process-group supervision, used in `src/spawn/process_group.rs`. |
| `thiserror` | `2.0.20` | Every fallible operation returns a typed error enum (L8 rule) — `SpawnError`/`JoinError`/`ProvisionError` all derive `thiserror::Error`. |
| `serde` | `1` (features `derive`) | Derives for the fd-3 packet shapes and `HermeticProvision`/scorecard structures. |
| `serde_json` | `1.0.151` | fd-3 packet (de)serialization, `LaneOutcome::Done.body`, `HermeticProvision.mcp_tool_manifest`. |
| `fs2` | `0.4.3` | `Scorecard`'s file-locked read-modify-write (`src/scorecard.rs`/`src/scorecard_io.rs`), ported from `agent.rs`. |
| `toml` | `0.8` | Reading the `.fleet/` tree's skill/agent declaration files (`src/sandbox/skills_registry.rs`, `src/sandbox/agent_registry.rs`). |
| `tempfile` | `3.27.0` | Per-lane hermetic tempdir construction (`src/sandbox/hermetic_env.rs`) — a normal (not dev-only) dependency in the built crate, since hermetic provisioning needs it at runtime, not just in tests. |

**Dropped from the pre-build proposal, confirmed absent from the built `Cargo.toml`:**
`command-group` (whole-process-tree kill stayed on the hand-rolled `libc`/`nix` `kill(-pid, ...)`
pattern), `rmcp` (the MCP tool-manifest types in `src/sandbox/manifest.rs` are hand-rolled
`serde_json::Value` construction, not `rmcp`'s model types), `cedar-policy` (flagged in the
pre-build note below as having no concrete use — confirmed still unused), and `tokio` (no async
variant of `spawn`/`join` was added; the crate stayed fully synchronous).

**Runtime (not compile-time) dependency:** `fleet-crew` (Python package at `crates/fleet-crew/`) —
invoked as a subprocess via `python3`, never linked or imported as a Rust crate. A missing/broken
`crew` environment surfaces as `SpawnError::ProcessSpawnFailed` / `LaneOutcome::EnvironmentFault`,
never a compile-time failure.

## 8. Crate file layout

> **HARD RULE: every source file ≤ 80 lines.** The fd-3 protocol alone
> (`spawn_agent_with_args`+`wait_with_deadline`+`terminate_group`) is ~185 lines in `main.rs`
> (3040-3192, plus helpers) — split by concern exactly as the task brief names them: adapter /
> spawn / sandbox / inject / fd3 / probe / scorecard.
>
> **As built** (`find crates/fleet-worker -name '*.rs' \| sort`, line counts via `wc -l`), the split
> landed differently from the pre-build sketch below in three ways: (1) there is no `inject/`
> directory — system-prompt assembly folded into `sandbox/`; (2) `spawn/worktree.rs` does not
> exist — worktree create/remove/naming moved to the `fleet-merge` crate dependency (§7) and
> `fleet-worker` calls it directly instead of vendoring a copy; (3) a `freelane/` module was added
> (not named in the pre-build sketch at all) for the keyless CLI fallback, plus a `#[[bin]]`
> fixture binary (`fw-fixture-agent`) and an `assets/` dir carrying the freelane shell script.

```
crates/fleet-worker/
  Cargo.toml
  assets/
    freelane.sh          # 259 — keyless CLI fallback script, shelled out to by the freelane/ module
    lanes.conf            # lane concurrency config consumed by freelane.sh
  src/
    lib.rs                 # 32 — module decls + re-exports
    adapter.rs              # 80 — CliAdapter, cli_binary_name()
    request.rs               # 63 — SpawnRequest, LaneHandle, SpawnError, JoinError (data only)
    outcome.rs                # 32 — LaneOutcome, NoAmbientProbeResult (data only)
    probe.rs                   # 34 — probe_no_ambient()
    scorecard.rs                # 49 — Scorecard type + record_scorecard_outcome()
    scorecard_io.rs               # 49 — file-locked read/write of the scorecard file
    scorecard_tests.rs              # 27 — scorecard unit tests
    freelane/
      mod.rs                        # 21 — re-exports
      embed.rs                       # 8 — embedded freelane.sh asset
      error.rs                        # 19 — freelane-specific error type
      materialize.rs                   # 22 — writes the embedded script to a runnable temp path
      output.rs                         # 60 — parses freelane's stderr markers (resolved_model/usage)
      root.rs                            # 53 — resolves the freelane root/working dir
      run.rs                              # 74 — shells out to freelane.sh, sends the fd-3 packet
    spawn/
      mod.rs                        # 80 — spawn(): calls fleet_merge::create, provisions the sandbox,
                                     #      builds and launches the child command
      child_command.rs               # 25 — builds the `<current_exe> __agent ...` Command
      process_group.rs                # 53 — setsid/setpgid + wait-with-deadline + terminate_group
      fd3.rs                           # 62 — socketpair setup, dup2 pre_exec
      fd3_send.rs                       # 53 — fd-3 send-side helpers
      fd3_recv.rs                        # 62 — fd-3 recv + validate_submission
      interpret.rs                        # 43 — interprets a validated packet into LaneOutcome
      join_impl.rs                         # 52 — join(): wait, drain fd-3, tear down via fleet_merge::remove
      util.rs                               # 19 — small shared helpers
    sandbox/
      mod.rs                        # 52 — re-exports resolve_hermetic_provision()
      hermetic_env.rs                 # 75 — per-lane tempdir, HOME/XDG_* redirection, env_clear()+allowlist
      manifest.rs                      # 66 — lease-scoped MCP tool manifest (hand-rolled JSON, not rmcp)
      manifest_tests.rs                  # 16 — manifest unit tests
      skills_registry.rs                  # 60 — Registry/Resolution, .fleet/-tree loading
      skills_registry_tests.rs              # 60 — skills-registry unit tests
      agent_registry.rs                      # 52 — agent identity/capability lookup by agent_id
      config_source.rs                        # 32 — reads the repo's committed .fleet/ config tree
      scaffold.rs                              # 53 — scaffolds a lane's sandbox directory tree
      scaffold_tests.rs                          # 33 — scaffold unit tests
      templates.rs                                # 13 — system-prompt / file templates
  tests/
    common/mod.rs               # 56 — shared test harness helpers
    fixtures/
      fixture_agent.rs           # 27 — fixture agent used by the fw-fixture-agent [[bin]]
      fixture_scenarios.rs        # 53 — scripted fixture behaviors (done/refuse/timeout/malformed)
    spawn_join_happy.rs          # 42 — successful lane end-to-end against the fixture CLI
    spawn_join_refusals.rs       # 68 — empty task, missing CLI, malformed fd-3
    timeout_kill.rs               # 48 — deadline exceeded escalates to whole-process-group kill
    hermetic_isolation.rs         # 69 — asserts spawned env never contains real HOME/XDG values
    scorecard_concurrency.rs      # 29 — concurrent record_scorecard_outcome calls don't lose updates
    freelane_live.rs              # 57 — exercises assets/freelane.sh end-to-end (restored after being
                                   #      wrongly deleted during an earlier cleanup pass)
```
> If any file above still projects over 80 lines once bodies land, split again. The file-size gate
> (§10) is run before Opus review; `src/spawn/mod.rs` and `src/adapter.rs` are already at the 80-line
> ceiling.

`Cargo.toml` (as built):
```toml
[package]
name = "fleet-worker"
version = "0.1.0"
edition = "2021"

[dependencies]
fleet-types = { path = "../fleet-types" }
fleet-merge = { path = "../fleet-merge" }
libc = "0.2.189"
nix = { version = "0.29", features = ["process", "resource", "signal"] }
thiserror = "2.0.20"
serde = { version = "1", features = ["derive"] }
serde_json = "1.0.151"
fs2 = "0.4.3"
toml = "0.8"
tempfile = "3.27.0"

[[bin]]
name = "fw-fixture-agent"
path = "tests/fixtures/fixture_agent.rs"
test = false
doc = false
```

## 9. Test plan

**Unit tests:**
- `cli_binary_name_is_none_only_for_freelane` — asserts `Freelane.cli_binary_name() == None` and
  `Claude`/`Codex` return `Some("claude")`/`Some("codex")`.
- `validate_submission_rejects_wrong_schema_version` — ports `main.rs::validate_submission`'s
  implicit contract: a packet with `schema_version: "2.0"` is rejected.
- `validate_submission_rejects_unknown_kind` — a packet with `kind: "unexpected"` is rejected.
- `unique_worktree_names_never_collide_under_the_same_pid` — ports
  `worktree.rs::two_concurrent_names_never_collide`.
- `scorecard_refresh_rejects_inconsistent_totals` — ports `agent.rs`'s implicit
  `checked == credited+faulted` / `total == checked+unknown` invariant check from
  `load_scorecard`/`record_scorecard_outcome`: a hand-constructed `Scorecard` with a wrong `total`
  is rejected on load, not silently accepted.
- `unknown_outcome_is_published_but_never_credited` — ports `agent.rs`'s
  `unknown_is_published_but_never_credited` test verbatim.

**Integration tests:**
- `spawn_join_stub_agent_round_trips_cleanly` — spawn a `stub`-shaped test CLI (a tiny fixture
  binary or the `__agent stub` path if `src/` is available in the test harness) that writes a
  known fd-3 `done` packet, `join` it, assert `LaneOutcome::Done` with the exact body; assert the
  worktree directory no longer exists afterward (mirrors `worktree.rs::create_and_remove_round_trips_cleanly`).
- `spawn_join_missing_cli_refuses_before_any_io` — `spawn` with `CliAdapter::Claude` on a `PATH`
  with no `claude` binary returns `Err(SpawnError::CliNotOnPath(..))` and creates NO worktree
  (assert the `.worktrees/` dir gained no new entry) — proves the pre-flight check actually runs
  before IO, not just that it eventually errors.
- `spawn_join_empty_task_refuses_before_any_io` — same shape, for `SpawnError::EmptyTask`.
- `join_times_out_and_kills_the_whole_process_group` — spawn a fixture that forks a
  long-sleeping grandchild and never writes fd-3; assert `join` returns
  `LaneOutcome::EnvironmentFault` after approximately `deadline` (not instantly, not much later)
  AND assert (via `ps`/a marker file the grandchild would have written) that the grandchild was
  also killed, not just the direct child — this is the regression test for `terminate_group`'s
  `-pid` (whole group) behavior, not just the child pid.
- `join_rejects_malformed_fd3_packet_as_environment_fault_not_refusal` — a fixture that sends
  non-JSON bytes on fd-3 → `LaneOutcome::EnvironmentFault`, explicitly asserting it is NOT
  `LaneOutcome::Refused` (the "first-empty-wins"-style category-confusion mutation this crate must
  guard against, analogous to `fleet-router`'s stage-ordering mutation target).
- `hermetic_spawn_never_exposes_real_home_or_xdg` — set `HOME`/`XDG_CONFIG_HOME` in the TEST
  process's own env to sentinel values, spawn a fixture agent that dumps its own environment
  (mirrors `main.rs`'s existing `"env-probe"` agent arm, reused as a diagnostic fixture here), and
  assert the child's reported `HOME`/`XDG_*` are NEITHER the sentinel values NOR unset, but a
  fresh per-lane tempdir path this crate created — the direct regression test for the brief's
  "NEVER the user's `~/.claude`" requirement.
- `probe_no_ambient_reports_clean_block_for_a_credential_requiring_stub` — against a fixture CLI
  that always fails without a specific env var this crate deliberately does not pass through,
  assert `NoAmbientProbeResult::CleanlyBlocked`.

**Mutation-testing targets (`cargo mutants -p fleet-worker`):**
- Flipping `child_fd != 3` to always-true (or always-false) in the `dup2` pre_exec guard must be
  killed by `spawn_join_stub_agent_round_trips_cleanly` — a wrong guard either double-dups or
  leaks the wrong fd, breaking the round trip.
- Deleting the `env_clear()` call (so the child inherits the parent's FULL environment instead of
  the allowlist) must be killed by `hermetic_spawn_never_exposes_real_home_or_xdg` — this is the
  single highest-value mutant in this crate given the HERMETIC requirement is the whole reason it
  exists.
- Changing `terminate_group`'s `kill(-pid, ...)` to `kill(pid, ...)` (dropping the negation, so
  only the direct child is signaled, not the group) must be killed by
  `join_times_out_and_kills_the_whole_process_group`'s grandchild-survival check.
- Swapping `validate_submission`'s `kind` match arms (accepting an unlisted kind) must be killed
  by `validate_submission_rejects_unknown_kind`.
- Flipping `scorecard.checked != credited+faulted` from `!=` to `==` in the consistency guard (so
  a corrupt scorecard is silently accepted) must be killed by
  `scorecard_refresh_rejects_inconsistent_totals`.

**Property tests (optional but recommended):**
- *Worktree names are pairwise distinct under N concurrent callers*: generate N (`N` up to 200)
  concurrent `unique_name` calls from simulated distinct "processes" (distinct pid inputs) and
  assert all N names are pairwise distinct — a direct generalization of
  `two_concurrent_names_never_collide`, guarding against a future change to the naming scheme that
  reintroduces a collision window.

## 10. Verification recipe

```bash
cd crates/fleet-worker
cargo test -p fleet-worker --all-targets
cargo clippy -p fleet-worker --all-targets -- -D warnings
cargo mutants -p fleet-worker
find src tests -name '*.rs' -exec wc -l {} + | awk '$1>80{print; f=1} END{exit f}'  # must print nothing
```
Expected: all unit + integration tests pass, 0 skipped — publish as `<passed>/<total>` (e.g.
`19/19`, not "tests pass"). Clippy: 0 warnings. Mutants: every target named in §9 caught; publish
`<caught>/<total mutants>` — given this crate's IO surface (subprocess spawn, real filesystem,
timing-dependent kill behavior), 100% mutant-kill is a harder bar than `fleet-router`'s pure-logic
100%; a floor of **≥90% caught, every named §9 target caught with zero exceptions** is this
crate's published floor — any survivor outside the named targets gets triaged by Opus, not silently
accepted.

Because this crate spawns real subprocesses and creates real git worktrees, its integration tests
additionally require: a `git` binary on `PATH`, a scratch git repo created via `tempdir()` (never
the actual project tree — L8 checklist), and either a real `claude`/`codex` CLI OR a test fixture
binary standing in for one (recommended: a tiny Rust fixture binary built as a
`[[bin]]` under `dev-dependencies`-equivalent, invoked by `spawn` in tests via a
`CliAdapter`-bypassing test-only constructor — **flag for Opus**: the public API in §3 has no such
test seam today; adding one is likely necessary and should be reviewed as part of this crate's
first real diff, not deferred).

## 11. L8 checklist

- [ ] Every fallible path returns a typed error enum (`SpawnError`/`JoinError`/`ProvisionError`) —
      none swallowed into `bool`/`Option`/`String`/`.unwrap()` in non-test code. (Mark done once
      built — the blueprint's signatures already commit to this; verify no `unwrap()` snuck into a
      real implementation.)
- [ ] Clock/RNG/IO are injected where feasible — **partially**: subprocess spawn and filesystem
      access are direct (§4 names this explicitly and flags it as an open question for Opus, not a
      silent gap); the worktree-naming jitter is dependency-free by design, matching
      `worktree.rs`'s existing choice.
- [ ] Thread-safety documented: `spawn`/`join` are safe to call from multiple threads
      concurrently (each call is independent — distinct worktree, distinct socketpair, distinct
      child process); `record_scorecard_outcome`'s file lock is the ONLY shared-mutable-state
      boundary in this crate, and it is already safe under concurrent same-agent calls (that is
      the entire reason the file lock exists — D54b's fix).
- [ ] No float used for money, tokens, or any precision-sensitive count — `Tokens` (from
      `fleet-types`) is used throughout; this crate's own `parse_freelane_usage`-equivalent must
      preserve the `prompt+completion==total` integer cross-check, never coerce to float.
- [ ] No self-grading — verification runs `cargo mutants`, not just unit tests; denominator
      published per §10 (mark done once actually run and recorded in the PR).
- [ ] The verify command's pass/fail denominator is stated in this file (§10) — restate the real
      numbers in the PR once built (mark done then).
- [ ] Tests that touch the filesystem: EVERY test-created worktree/tempdir uses `tempfile::tempdir()`
      or a `TempDir`-scoped git init, NEVER the repo tree or `$HOME` — this is doubly important
      here versus other crates, since this crate's entire job is filesystem/subprocess isolation;
      a test that accidentally touches the real repo would be a direct contradiction of what this
      crate claims to guarantee.
- [ ] Every non-goal in §2 is actually absent from the code: no ledger `append_receipt` call, no
      token-quota/cooldown read, no CLI-arg parsing, no role safety-gate re-evaluation, no in-process
      execution of `fleet-crew` Python code (only subprocess invocation) — enforceable via
      `grep -rn 'append_receipt\|env::var("FLEET_ROUTE_COOLDOWNS")' crates/fleet-worker/src/` must
      return nothing.
- [ ] **No source file exceeds 80 lines** (verified: `find src tests -name '*.rs' | xargs wc -l` —
      every file ≤ 80). `lib.rs` is a thin hub, not a dumping ground.

## 12. Definition of Done

`fleet-worker` is DONE when: §10's exact commands all pass with a published denominator (tests
`N/N`, clippy clean, mutants `M/M` caught with every §9 target present), every unchecked box in §11
is checked with its real numbers, `registry/services/REGISTRY.md` lists the crate, AND Opus has (a)
re-derived the fd-3 protocol and hermetic-provisioning contract from this blueprint alone, (b)
reproduced the `env_clear()`-deletion mutation by hand and confirmed
`hermetic_spawn_never_exposes_real_home_or_xdg` catches it, (c) driven one real `spawn`→`join` call
end-to-end against an actual CLI (not just the stub fixture) confirming a `LaneOutcome::Done` is
produced with a real `resolved_model`, and — because this crate carries the roster's single
highest-risk unknown — (d) reviewed `probe_no_ambient`'s result for at least one real CLI (`claude`
and/or `codex`) and personally judged whether `NoAmbientProbeResult::UnexpectedSuccess` (if it
occurs) represents a genuine keyless capability or a credential leak this crate failed to block.
Step (d) is NOT satisfiable by a passing test suite alone — it requires a human/Opus judgment call
on real CLI behavior, which is exactly why this is flagged as the crate's highest-risk unknown
rather than an ordinary acceptance criterion.

**Highest-risk unknown (flagged per the task brief's instruction):** whether the keyless/hermetic
thesis — that a CLI can be driven with zero ambient credential exposure, `env_clear()`'d and
`HOME`/`XDG_*`-redirected — actually holds for `claude`/`codex`'s real, current CLI implementations,
or whether one or both silently probe a fixed path outside what this crate controls (e.g. a
platform keychain, a hardcoded `~`-relative cache independent of `$HOME`, or a background daemon
already running with the real user's credentials that the spawned CLI talks to over a local socket
instead of reading files at all). `probe_no_ambient` is this blueprint's answer to "how do we find
out," but the fn's own doc comment is explicit that an `UnexpectedSuccess` result requires human
judgment to classify — no test in §9 can fully automate this, because the failure mode by
definition looks identical to legitimate keyless capability (`freelane`) from inside this crate's
own vantage point. This is the M1 experiment the task brief names, and it is real, unresolved risk,
not a gap in this blueprint's diligence.

---

## Divergence from MIGRATION-PLAN (for Opus)

MIGRATION-PLAN §3 row 13 describes `fleet-worker` as `refactor` reuse from `worktree.rs` ·
`skills.rs` · `mcp.rs` · `agent.rs` · crew adapters · fd-3 in `main.rs`. Having read all five
sources in full, several things diverge from that framing:

1. **This is a parent/child split, not a single-process lift.** The fd-3 protocol in `main.rs`
   spans TWO sides of a `fork`+`exec`: the parent (`spawn_agent_with_args`, `wait_with_deadline`)
   which this crate owns, and the child (`agent_command`'s dispatch, `run_model_agent`,
   `run_freelane_agent`) which today runs as `fleet __agent <kind> ...` via
   `env::current_exe()` — necessarily inside the SAME binary as the composition root, because only
   that binary can re-exec itself post-`dup2`. This blueprint assigns only the parent side to
   `fleet-worker`; the child-side dispatch stays in `src/`. MIGRATION-PLAN doesn't say this
   explicitly — worth a line in §3 row 13 and in the DAG note, since it means `fleet-worker` and
   `src/`'s `__agent` subcommand share the fd-3 WIRE PROTOCOL as a contract without either one
   containing the other's code.

2. **`worktree.rs` is genuinely shared with `fleet-merge` (MIGRATION-PLAN row 9), and the plan
   doesn't say who owns the canonical copy.** Both crates need "create an isolated worktree, later
   remove it" — this blueprint takes the position that `fleet-worker` needs it for spawning lanes
   and `fleet-merge` needs it for merge staging, and recommends BOTH vendor the ~230-line primitive
   independently rather than one importing the other (worktree creation has zero decision logic
   worth centralizing, and a sibling→sibling edge here would violate the DAG's "sibling edges must
   be minimal and named" rule for very little benefit) — but flags this as a call only Opus should
   make explicitly, not something this blueprint should decide unilaterally by omission.

3. **`agent.rs`'s `Scorecard` re-homing (per MIGRATION-PLAN §7's citation "per MIGRATION-PLAN §7")
   is confirmed correct** — `record_scorecard_outcome`'s file-locked read-modify-write fixing D54b
   is exactly `fleet-worker`'s concern (recording an outcome per completed lane) and is NOT claimed
   by any other crate in the roster. This part of the task brief's framing holds up under reading
   the source.

4. **`agent.rs`'s `Assignment<S>` type-state machine (lines 291-476) is NOT part of this crate**,
   despite `agent.rs` being named in the task brief — it is a parallel instance of the same
   type-state pattern MIGRATION-PLAN's `fleet-lifecycle` crate (added in §7's teach-back log,
   "orphan-rule" note) was created to own. This blueprint recommends Opus decide whether
   `Assignment<S>` unifies with `fleet-lifecycle`'s `Task<S>` or stays a second, smaller machine —
   either way, it does not belong in `fleet-worker`, which only calls `record_scorecard_outcome`
   as a fire-and-forget fact-recording call, never manages an agent's assignment lifecycle.

5. **The task brief lists `cedar-policy` as a dependency, but no concrete use for a policy-
   evaluation engine emerged from reading the five source files.** `resolve_hermetic_provision`'s
   skill/capability resolution (from `skills.rs`) is plain set-membership, not a policy language.
   This blueprint recommends deferring `cedar-policy` until a specific authorization requirement
   names it (e.g. a future lease-scoping richer than prefix-matching) rather than adding an unused
   dependency now — flagged as a departure from the brief, not silently dropped.

6. **The brief's "HOME/XDG overridden to a per-lane tempdir" requirement is NOT what `main.rs`
   does today.** Today's `spawn_agent_with_args` PASSES THROUGH the parent's real `HOME`
   (`env::var("HOME").ok()` then `command.env("HOME", value)`) — it does not redirect it. This is
   the single most load-bearing piece of NEW logic in this blueprint versus a straight port: every
   other ported fd-3 mechanic (socketpair, dup2, setsid, wait/kill, validate_submission) is a
   verbatim lift; the HOME/XDG redirection is genuinely new code this crate must write, tested by
   `hermetic_spawn_never_exposes_real_home_or_xdg` (§9) — flagged so Opus does not mistake this for
   an already-proven piece of the port.
