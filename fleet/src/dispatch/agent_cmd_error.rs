//! `AgentCmdError` -- everything `__agent` can fail on, each mapped to a distinct `ExitCode` so
//! a human running it by hand (exactly how the owner found this bug) gets a clear diagnostic
//! instead of a panic or a silent success.

use fleet_types::ExitCode;
use fleet_worker::UnknownAgentKind;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum AgentCmdError {
    /// The `<kind>` token didn't match any known `CliAdapter` (`fleet-worker`'s
    /// `agent_kind()`/`from_agent_kind()` inverse pair).
    #[error(transparent)]
    UnknownKind(#[from] UnknownAgentKind),
    /// `<worktree>` does not exist or is not a directory.
    #[error("worktree path {0:?} does not exist or is not a directory")]
    MissingWorktree(PathBuf),
    /// `<task>` was empty/whitespace-only -- the parent (`fleet_worker::spawn`) already refuses
    /// this before ever spawning a child, so seeing it here means that contract was bypassed.
    #[error("task is empty or all-whitespace")]
    EmptyTask,
    /// fd 3 is not a valid, open file descriptor: `__agent` was run directly from a shell
    /// instead of spawned by `fleet-worker`, which is the exact way the owner found this bug.
    #[error(
        "fd 3 is not open -- `__agent` is not meant to be run by hand, it is the child-side \
         re-exec target `fleet-worker` spawns with a receipt channel already wired to fd 3"
    )]
    Fd3NotOpen,
    /// The receipt packet itself failed to send on fd 3.
    #[error("failed to send fd-3 receipt: {0}")]
    SendFailed(#[from] std::io::Error),
    /// The adapter's own work genuinely could not be done (missing CLI, missing script, a
    /// non-zero worker exit) -- a `refuse` packet was already sent on fd 3 before this returns.
    #[error("{0}")]
    Refused(String),
}

impl AgentCmdError {
    pub fn exit_code(&self) -> ExitCode {
        match self {
            AgentCmdError::UnknownKind(_) => ExitCode::Refusal,
            AgentCmdError::MissingWorktree(_) => ExitCode::Env,
            AgentCmdError::EmptyTask => ExitCode::Invariant,
            AgentCmdError::Fd3NotOpen => ExitCode::Mismatch,
            AgentCmdError::SendFailed(_) => ExitCode::Env,
            AgentCmdError::Refused(_) => ExitCode::Refusal,
        }
    }
}
