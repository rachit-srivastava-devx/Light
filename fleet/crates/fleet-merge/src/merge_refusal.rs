//! `MergeRefusal`: why `merge_lane` refused. Split out of `error.rs` to keep `WorktreeError`
//! (which gained a `NotFound` variant) under the file's line budget; each variant is one of
//! `merge-lane.sh`'s already-shipped refusals.

use fleet_types::ExitCode;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum MergeRefusal {
    /// `merge-lane.sh:9`. No directory at the given worktree path.
    #[error("no worktree at {0}")]
    NoWorktree(PathBuf),
    /// `merge-lane.sh:10`. `git add -A` itself failed (not "staged nothing" -- errored).
    #[error("git add failed in {0}")]
    StageFailed(PathBuf),
    /// `merge-lane.sh:12-14`, the D31 fix itself: staged exactly 0 files.
    #[error("lane {branch} staged 0 files -- it produced nothing")]
    EmptyStage { branch: String },
    /// `merge-lane.sh:15-16`. The lane's own commit failed.
    #[error("commit failed for lane {branch}")]
    CommitFailed { branch: String },
    /// `merge-lane.sh:18`. `git merge` reported a conflict.
    #[error("conflict merging lane {branch}")]
    Conflict { branch: String },
    /// `merge-lane.sh:20`. `git merge` exited 0 but `HEAD` is byte-identical before/after.
    #[error("merging lane {branch} moved HEAD nowhere -- nothing was integrated")]
    HeadUnmoved { branch: String },
    /// `merge-lane.sh:22`. `HEAD` moved but the diff between before/after touches 0 files.
    #[error("merging lane {branch} changed 0 files")]
    NoFilesChanged { branch: String },
    /// The `git` process itself could not be spawned.
    #[error("could not spawn git: {0}")]
    Spawn(String),
}

impl MergeRefusal {
    /// `NoWorktree` is `exit 3` (`ExitCode::Env`); every other refusal is `exit 6` (`Invariant`).
    pub fn exit_code(&self) -> ExitCode {
        match self {
            MergeRefusal::NoWorktree(_) => ExitCode::Env,
            _ => ExitCode::Invariant,
        }
    }
}
