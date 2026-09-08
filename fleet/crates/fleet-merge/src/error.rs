//! Typed errors, mapping to fleet's `ExitCode` taxonomy instead of a 15th redeclaration of it.
//! `MergeRefusal` lives in `merge_refusal.rs` -- kept separate so this file stays small.

use fleet_types::ExitCode;
use std::path::PathBuf;

pub use crate::merge_refusal::MergeRefusal;

/// Why `create` or `remove` failed. Mirrors `worktree.rs`'s `EXIT_ENV`/`EXIT_INVARIANT` split.
#[derive(Debug, thiserror::Error)]
pub enum WorktreeError {
    /// `name` was empty or all-whitespace (`worktree.rs:53-55`).
    #[error("worktree name must not be empty")]
    EmptyName,
    /// `git worktree add` failed on every one of the 8 jittered-retry attempts.
    #[error("git worktree add failed after {attempts} retries: {stderr}")]
    CreateFailed { attempts: u32, stderr: String },
    /// `git worktree add` reported success but the target directory does not exist on disk.
    #[error("git reported success but {path} does not exist")]
    MissingAfterCreate { path: PathBuf },
    /// `git worktree remove --force` and the filesystem fallback both left the dir present.
    #[error("worktree at {path} could not be removed -- it still exists on disk")]
    RemoveLeaked { path: PathBuf },
    /// `remove` was asked to remove a worktree whose directory does not exist on disk. Distinct
    /// from a successful removal: the caller asked for something that was never there, and must
    /// not be told `ok` as if their request took effect.
    #[error("no worktree exists at {path} -- nothing to remove")]
    NotFound { path: PathBuf },
    /// The `git` process itself could not be spawned (binary missing, permissions).
    #[error("could not spawn git: {0}")]
    Spawn(String),
    /// `remove` was handed a path that is NOT inside `<repo>/.worktrees/`. Refusing is the whole
    /// point: `remove`'s fs fallback is a recursive delete, so before this guard existed
    /// `fleet rollback --repo <any repo> --worktree /any/path` destroyed that path and printed
    /// "ok: worktree removed" with exit 0. Proven against a scratch dir, 2026-09-08.
    #[error("refusing to remove {path}: not inside {root} -- fleet only removes worktrees it owns")]
    OutsideWorktreeRoot { path: PathBuf, root: PathBuf },
    /// The recursive fallback delete itself failed. Previously `let _ = remove_dir_all(..)`, so a
    /// partial delete was silently ignored.
    #[error("removing {path} failed: {source_msg}")]
    RemoveFailed { path: PathBuf, source_msg: String },
}

impl WorktreeError {
    /// Maps to fleet's exit-code contract: `EXIT_ENV = 3` / `EXIT_INVARIANT = 6` by meaning.
    pub fn exit_code(&self) -> ExitCode {
        match self {
            WorktreeError::EmptyName
            | WorktreeError::MissingAfterCreate { .. }
            | WorktreeError::RemoveLeaked { .. } => ExitCode::Invariant,
            WorktreeError::CreateFailed { .. }
            | WorktreeError::Spawn(_)
            | WorktreeError::NotFound { .. } => ExitCode::Env,
            // A deliberate "no": the caller asked fleet to delete something outside its sandbox.
            WorktreeError::OutsideWorktreeRoot { .. } => ExitCode::Refusal,
            WorktreeError::RemoveFailed { .. } => ExitCode::Invariant,
        }
    }
}
