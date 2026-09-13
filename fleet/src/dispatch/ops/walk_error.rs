//! `WalkError` -- split out of `walk.rs` to keep that file under the line budget.

use std::path::PathBuf;
use std::time::Duration;

/// Why the walk stopped short of a complete result -- typed so a caller can tell a resource
/// budget from a real IO failure, never spinning forever waiting for either.
#[derive(Debug, thiserror::Error)]
pub enum WalkError {
    #[error("walk exceeded the {0}-file budget while scanning {1:?}")]
    TooManyFiles(usize, PathBuf),
    #[error("walk exceeded the {0}-byte budget while scanning {1:?}")]
    TooManyBytes(u64, PathBuf),
    #[error("walk exceeded its {0:?} wall-clock deadline while scanning {1:?}")]
    DeadlineExceeded(Duration, PathBuf),
    /// The repo root itself does not exist or could not be read (typo'd `--repo`, permissions).
    /// Distinct from a subdirectory read failure mid-walk (skipped, matching prior behaviour):
    /// this is the ONE directory the caller explicitly named, so silently answering "0 files"
    /// for it would print a confident, wrong `matching_symbols: 0` indistinguishable from a
    /// real zero-hit query against a real repo.
    #[error("repo root {0:?} does not exist or could not be read: {1}")]
    RepoUnreadable(PathBuf, String),
}
