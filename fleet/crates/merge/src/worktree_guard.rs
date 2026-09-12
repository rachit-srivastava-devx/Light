//! The containment guard for `worktree::remove`.
//!
//! `remove` falls back to a RECURSIVE DELETE when `git worktree remove` fails, and it used to apply
//! that delete to the caller-supplied path unvalidated. So `fleet rollback --repo <any git repo>
//! --worktree /any/existing/path` destroyed that path and printed `ok: worktree removed` with exit
//! 0 -- proven against a scratch dir on 2026-09-08 (file gone, dir gone, exit 0).
//!
//! This is the one place that decides whether a path is fleet's to delete. Both sides are
//! canonicalised first, so `..` and symlinks cannot walk out: a bare `starts_with` on unresolved
//! paths would accept `<repo>/.worktrees/../../../etc`.

use crate::error::WorktreeError;
use std::path::{Path, PathBuf};

/// Fleet only creates worktrees under `<repo>/.worktrees/<name>`, so anything else is not fleet's
/// to remove. Returns the canonical path. Refuses rather than deleting when the path escapes the
/// root, IS the root (removing every worktree is not "remove this worktree"), or cannot be
/// canonicalised -- an unresolvable path is not permission to recurse into it.
pub fn ensure_owned_worktree(repo: &Path, path: &Path) -> Result<PathBuf, WorktreeError> {
    let root = repo.join(".worktrees");
    // The refusal always names the path the CALLER passed and the root it had to be under --
    // never a canonicalisation artefact. An earlier version printed the root as the subject and
    // leaked `<unresolvable: ...>`, which told the operator nothing about their own command.
    let refuse = || WorktreeError::OutsideWorktreeRoot {
        path: path.to_path_buf(),
        root: root.clone(),
    };
    // Root absent => this repo has no worktrees at all, so nothing here is fleet's to delete.
    let (root_c, path_c) = match (root.canonicalize(), path.canonicalize()) {
        (Ok(r), Ok(p)) => (r, p),
        _ => return Err(refuse()),
    };
    if path_c == root_c || !path_c.starts_with(&root_c) {
        return Err(refuse());
    }
    Ok(path_c)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn repo() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join(".worktrees/lane-a")).unwrap();
        d
    }

    #[test]
    fn accepts_a_worktree_fleet_actually_owns() {
        let d = repo();
        let ok = ensure_owned_worktree(d.path(), &d.path().join(".worktrees/lane-a"));
        assert!(ok.is_ok(), "{ok:?}");
    }

    /// The exact exploit: an unrelated directory handed in via `--worktree`.
    #[test]
    fn refuses_an_unrelated_directory_instead_of_deleting_it() {
        let d = repo();
        let victim = tempfile::tempdir().unwrap();
        let err = ensure_owned_worktree(d.path(), victim.path()).unwrap_err();
        assert!(matches!(err, WorktreeError::OutsideWorktreeRoot { .. }), "{err:?}");
        assert!(victim.path().exists(), "guard must not delete anything itself");
    }

    /// A bare `starts_with` on unresolved paths would accept this.
    /// `..` traversal, the root itself, and an unresolvable path. A bare `starts_with` on
    /// unresolved paths would accept the first of these.
    #[test]
    fn refuses_traversal_the_root_itself_and_unresolvable_paths() {
        let d = repo();
        for p in [".worktrees/lane-a/../../..", ".worktrees"] {
            assert!(ensure_owned_worktree(d.path(), &d.path().join(p)).is_err(), "{p}");
        }
        assert!(ensure_owned_worktree(d.path(), Path::new("/definitely/not/here")).is_err());
    }
}
