//! Containment guard for freelane's applied writes. Canonicalises the worktree and the resolved
//! target and requires the target to fall strictly inside it, so `../../../etc` -- and a
//! symlinked ancestor or the target itself pointing outside the worktree -- are refused rather
//! than followed. Same shape as `crates/fleet-merge/src/worktree_guard.rs::ensure_owned_worktree`
//! (that guard protects deletion, this one protects writes): a bare `starts_with` on unresolved
//! paths accepts `<worktree>/../../../etc`, so both sides are canonicalised first.

use super::error::ApplyError;
use std::path::{Component, Path, PathBuf};

pub fn resolve_target(worktree: &Path, fence_index: usize, raw: &str) -> Result<PathBuf, ApplyError> {
    let rel = Path::new(raw);
    if rel.is_absolute() {
        return Err(ApplyError::AbsolutePath { index: fence_index, path: raw.to_string() });
    }
    for component in rel.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            // ParentDir (`..`), RootDir, and Prefix are all refused as traversal -- a relative
            // path has no business containing any of them.
            _ => return Err(ApplyError::PathTraversal { index: fence_index, path: raw.to_string() }),
        }
    }
    let worktree_c = worktree
        .canonicalize()
        .map_err(|e| ApplyError::WorktreeUnresolvable(e.to_string()))?;
    let candidate = worktree_c.join(rel);
    ensure_no_escape(&worktree_c, &candidate, fence_index, raw)?;
    Ok(candidate)
}

/// Walks up from `candidate` to its nearest EXISTING ancestor (itself, if `candidate` already
/// exists) and canonicalises that. A symlinked directory on the way down, or the target itself
/// already existing as a symlink, resolves to its real location here even though the plain path
/// string never contained `..`.
fn ensure_no_escape(worktree_c: &Path, candidate: &Path, fence_index: usize, raw: &str) -> Result<(), ApplyError> {
    let mut existing = candidate.to_path_buf();
    while !existing.exists() {
        if !existing.pop() {
            break;
        }
    }
    let Ok(existing_c) = existing.canonicalize() else {
        // Unresolvable ancestor: fail closed, not open.
        return Err(ApplyError::EscapesWorktree { index: fence_index, path: raw.to_string() });
    };
    if existing_c != *worktree_c && !existing_c.starts_with(worktree_c) {
        return Err(ApplyError::EscapesWorktree { index: fence_index, path: raw.to_string() });
    }
    Ok(())
}
