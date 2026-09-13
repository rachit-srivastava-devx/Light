//! `ensure_repo` -- the S1 pt. 3 refusal: a nonexistent/unreadable `--repo` given to `fleet
//! gate`/`fleet oracle`/`fleet run`/`fleet swarm` must be a typed error and a non-zero exit,
//! never a silent fallback to the calling process's own cwd, and never a downstream stage
//! failing opaquely on a per-gate git command because the target directory is a workspace of
//! sibling checkouts rather than a repo itself. Reuses `walk::ensure_repo_readable` for the
//! nonexistent-directory case, then layers the git-worktree check that every downstream stage
//! (verify gates, worker worktree, merge) assumes.

use super::error::DispatchError;
use super::walk::ensure_repo_readable;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn ensure_repo(repo: &str) -> Result<PathBuf, DispatchError> {
    let path = Path::new(repo).to_path_buf();
    ensure_repo_readable(&path)?;
    ensure_git_worktree(&path)?;
    Ok(path)
}

/// True iff `git -C <path> rev-parse --show-toplevel` exits 0 -- accepts both a repo root
/// (`.git/`) and any directory INSIDE a git worktree (which is what `git`'s own commands
/// require, and what every downstream stage of `run`/`swarm` will invoke). A `.git` file (a
/// worktree checkout, not a directory) is also accepted by this check, unlike a hand-rolled
/// `path.join(".git").is_dir()`.
pub fn ensure_git_worktree(path: &Path) -> Result<(), DispatchError> {
    let ok = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if ok {
        return Ok(());
    }
    Err(DispatchError::EnvFault(format!(
        "--repo {}: not a git repository (nor a directory inside one). \
fleet's gates run against a git worktree; point --repo at the repo root, \
not a parent directory that just contains multiple checkouts as siblings.",
        path.display()
    )))
}

#[cfg(test)]
#[path = "verify_repo_tests.rs"]
mod tests;
