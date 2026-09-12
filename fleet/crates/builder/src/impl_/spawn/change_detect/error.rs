//! `ChangeDetectError` -- why `lane_changed` could not determine whether a lane's worktree
//! changed. Every variant is a `git` invocation problem (spawn failure or non-zero exit), never
//! "the worker did nothing" -- that distinction is the whole point: folding a git failure into
//! "0 changes" used to downgrade an ENVIRONMENT fault into a false `Refused` (AGENTS.md rule 7).

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum ChangeDetectError {
    /// `git status --porcelain` failed to run, or ran and exited non-zero (e.g. `path` is not
    /// inside a git worktree at all, or `git` itself is missing/broken).
    #[error("git status failed in {path}: {detail}")]
    StatusUnavailable { path: PathBuf, detail: String },
    /// `git rev-parse HEAD` failed to run, exited non-zero, or produced no sha -- covers a
    /// commit-less/unborn worktree and a mid-run git failure alike; both mean "cannot tell",
    /// never "unchanged".
    #[error("git rev-parse HEAD failed in {path}: {detail}")]
    HeadUnreadable { path: PathBuf, detail: String },
}
