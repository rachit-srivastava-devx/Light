//! `DispatchError` -- the seam wrapping every crate's own typed error for exit-code mapping
//! (BLUEPRINT §11: no `.map_err(|_| EXIT_CODE)` collapsing a real error into an opaque code).
//! `exit_code()` lives in `error_exit.rs` -- kept out of this file to hold the ≤80-line rule.

use types::ExitCode;

#[derive(Debug, thiserror::Error)]
pub enum DispatchError {
    #[error("router refused: {0:?}")]
    Router(route::Refusal),
    #[error("role check refused: {0}")]
    RoleCheck(&'static str),
    #[error(transparent)]
    Admit(#[from] control::AdmitError),
    #[error(transparent)]
    Settle(#[from] control::SettleError),
    #[error(transparent)]
    Meter(#[from] control::MeterIoError),
    #[error(transparent)]
    Spawn(#[from] builder::SpawnError),
    #[error(transparent)]
    Join(#[from] builder::JoinError),
    /// `GateRefusal` does not implement `std::error::Error` (it is a plain wire-shape struct),
    /// so this wraps it by value rather than via `#[from]`/`transparent`.
    #[error("lifecycle refused: {}", .0.code())]
    Lifecycle(types::GateRefusal),
    #[error(transparent)]
    Provision(#[from] builder::ProvisionError),
    #[error(transparent)]
    Merge(#[from] integrate::WorktreeError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("this subcommand's owning crate does not yet expose a public entry point: {0}")]
    NotYetImplemented(&'static str),
    #[error("{0}")]
    Refusal(String),
    /// A lane's fd-3 channel delivered nothing, a malformed packet, or the deadline fired. This is
    /// an ENVIRONMENT fault and must never be reported as an agent refusal (AGENTS.md rule 7).
    #[error("environment fault: {0}")]
    EnvFault(String),
    /// `fleet oracle`/`fleet gate` ran to completion but the aggregate verdict was not clean --
    /// the exit code is `Report::exit_code()`'s own, never collapsed to a fixed variant here.
    #[error("verification failed: {failed} failed, {skipped} skipped (of {total} gate(s))")]
    VerifyFailed {
        failed: usize,
        skipped: usize,
        total: usize,
        code: ExitCode,
    },
    /// `fleet gate --id <id>` where `<id>` matches no entry in `verify::GATES`.
    #[error("no gate matches id {0:?}")]
    UnknownGate(String),
    /// Resolving where gate scripts live failed -- a bad `$FLEET_GATES_ROOT` override, or the
    /// embedded copies could not be materialized to a temp dir.
    #[error(transparent)]
    GateAssets(#[from] verify::GateAssetError),
    /// `fleet pr_emit` failed.
    #[error(transparent)]
    PrEmit(#[from] integrate::PrError),
    /// `fleet graph|impact`'s source walk hit its file/byte/deadline budget (§ `walk.rs`).
    #[error(transparent)]
    Walk(#[from] crate::dispatch::walk::WalkError),
    /// `fleet __planahead_probe`'s `run_plan_ahead` failed.
    #[error(transparent)]
    PlanAhead(#[from] crate::pipeline::planahead::PlanAheadError),
    /// `fleet __agent`'s own child-side dispatch failed -- see `agent_cmd.rs`.
    #[error(transparent)]
    Agent(#[from] crate::dispatch::agent_cmd::AgentCmdError),
    /// `fleet adjudicate` failed -- see `adjudicate_cmd_error.rs`.
    #[error(transparent)]
    Adjudicate(#[from] crate::dispatch::adjudicate_cmd_error::AdjudicateCmdError),
}
