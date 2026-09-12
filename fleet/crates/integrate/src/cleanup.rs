use crate::IntegrateError;
use std::path::Path;
use std::process::Command;

/// Idempotent cleanup of stale worktree registrations under `repo/.worktrees/`.
///
/// MUST only be called after the receipt is durable.  Prunes git's internal
/// worktree list; removing a non-existent directory is a no-op.
pub fn cleanup_worktree(repo: &Path) -> Result<(), IntegrateError> {
    // Prune stale worktree registrations (no-op if none exist).
    let _ = Command::new("git")
        .args(["worktree", "prune"])
        .current_dir(repo)
        .output();
    let worktrees = repo.join(".worktrees");
    if !worktrees.exists() {
        return Ok(());
    }
    Ok(())
}
