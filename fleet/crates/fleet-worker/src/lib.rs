//! Spawn one CLI-driven agent inside an isolated worktree + hermetic sandbox, and collect its
//! authoritative result over the fd-3 receipt channel. See BLUEPRINT.md for the full contract.
//!
//! This crate performs real IO (subprocess spawn, socketpair, filesystem) by design. Every IO
//! boundary is named: worktree creation/removal (shared with `fleet-merge`, path-depended on
//! here per the task brief), subprocess spawn + fd-3 recv (this crate's core), and the hermetic
//! tempdir the sandbox lives under. Nothing here reads the user's real `HOME`/`~/.claude`/
//! `~/.codex` -- every credential/config path a spawned CLI sees is either absent (`env_clear()`)
//! or redirected into a throwaway per-lane directory this crate owns and deletes.

mod adapter;
pub mod freelane;

/// Tuned system prompt appended to `claude -p` invocations by
/// `fleet/src/dispatch/agent_cmd_run.rs`. Encodes the five named idioms Fleet has seen
/// Claude miss (React `useState`-initializer-once, explicit `Set<>` dedup, Safari
/// `revokeObjectURL` deferral, CSV formula-injection guard, stable sort tiebreak) plus
/// the "Fleet expects / anti-patterns / when you are done" discipline. Verbatim source
/// lives at `crates/fleet-worker/assets/claude-system-prompt.md`; re-test against a
/// benchmark task if you edit it.
pub const CLAUDE_SYSTEM_PROMPT: &str =
    include_str!("../assets/claude-system-prompt.md");
mod outcome;
mod probe;
mod reap;
mod reap_sweep;
mod request;
mod sandbox;
mod scorecard;
mod scorecard_io;
mod spawn;

pub use adapter::{CliAdapter, UnknownAgentKind};
pub use fleet_types::Role;
pub use outcome::{LaneOutcome, NoAmbientProbeResult};
pub use probe::probe_no_ambient;
pub use reap::find_dead_lanes;
pub use reap_sweep::reap_dead_lanes;
pub use request::{JoinError, LaneHandle, SpawnError, SpawnRequest};
pub use sandbox::{
    resolve_hermetic_provision, scaffold::scaffold_fleet_dir, scaffold::ScaffoldError,
    HermeticProvision, ProvisionError,
};
pub use scorecard::{Scorecard, ScorecardOutcome};
pub use scorecard_io::{scorecard_path, record_scorecard_outcome};
pub use fleet_merge::MergeOutcome;
pub use spawn::{fd3, join, spawn, MergePolicy};
