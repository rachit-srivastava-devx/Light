//! `SpawnRequest`/`LaneHandle` and the `SpawnError`/`JoinError` taxonomies. Data only -- no IO.

use crate::adapter::CliAdapter;
use fleet_types::{LaneId, Role, TaskId};
use std::path::PathBuf;
use std::time::Duration;

/// Everything needed to spawn one lane. The caller already resolved which adapter/model to use
/// (via `fleet-router`'s `Decision`); this crate does not re-derive that.
#[derive(Clone, Debug)]
pub struct SpawnRequest {
    pub repo: PathBuf,
    pub role: Role,
    pub task_id: TaskId,
    pub adapter: CliAdapter,
    /// `None` for `Freelane` (no model parameter) and `stub`-shaped test lanes.
    pub requested_model: Option<String>,
    /// Free-text task prompt. Empty/whitespace-only is refused before any IO.
    pub task: String,
    /// Wall-clock budget before this crate escalates to SIGTERM then SIGKILL of the whole
    /// process group.
    pub deadline: Duration,
}

/// A live, spawned lane. Dropping this WITHOUT calling `join` leaks the worktree and the
/// sandbox tempdir -- callers MUST pair every `spawn` with a `join`.
#[derive(Debug)]
pub struct LaneHandle {
    pub lane_id: LaneId,
    pub worktree_path: PathBuf,
    /// Private join-time state below -- not part of the §3 public contract, but `LaneHandle`
    /// must carry it somewhere for `join` to have anything to wait on.
    pub(crate) child: std::process::Child,
    pub(crate) parent_fd: std::os::unix::io::RawFd,
    pub(crate) repo: PathBuf,
    pub(crate) worktree: fleet_merge::Worktree,
    /// `HEAD` inside the worktree the moment `spawn` finished creating it -- the fixed point
    /// `join`'s honesty check (`change_detect`) compares the finished lane against to decide
    /// "did this lane's tree change", independent of whether the worker left uncommitted edits,
    /// committed, or committed-then-reset.
    pub(crate) base_commit: String,
    pub(crate) sandbox_root: PathBuf,
    pub(crate) deadline: Duration,
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
    /// `git rev-parse HEAD` inside the just-created worktree failed or produced no sha.
    /// Structurally should not happen (`fleet_merge::create` already required `HEAD` to resolve
    /// to get this far), but a silently wrong/empty base commit would make every later
    /// change-detection comparison lie, so this is a hard error, not a best-effort default.
    #[error("could not read HEAD inside the new worktree: {0}")]
    BaseCommitUnreadable(String),
    #[error("hermetic sandbox provisioning failed: {0}")]
    SandboxProvisionFailed(String),
    #[error("socketpair() failed to establish the fd-3 channel")]
    Fd3ChannelUnavailable,
    #[error("subprocess spawn failed: {0}")]
    ProcessSpawnFailed(String),
}

/// Why `join` could not cleanly tear down after a lane finished.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum JoinError {
    #[error("worktree/sandbox teardown failed after the lane finished: {0}")]
    TeardownFailed(String),
}
