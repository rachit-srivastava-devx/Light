//! Actually remove the lanes `reap::find_dead_lanes` proves are dead, through the SAME
//! containment-guarded `fleet_merge::remove` path `fleet rollback` already uses -- this never
//! adds a second way to delete a worktree, only a second way to decide one is safe to hand to
//! the existing one. Not wired into any CLI command yet: that is a `src/`-side change (out of
//! this crate's scope), e.g. calling this once at the top of every `fleet` invocation or from a
//! `fleet rollback --sweep`.

use crate::reap::find_dead_lanes;
use fleet_merge::WorktreeError;
use std::path::{Path, PathBuf};

/// One (path, outcome) pair per lane `find_dead_lanes` proved dead. Never touches a lane it did
/// not prove dead, and never a path outside `<repo>/.worktrees` -- `fleet_merge::remove`'s own
/// `ensure_owned_worktree` guard still applies underneath this, unweakened.
pub fn reap_dead_lanes(repo: &Path) -> Vec<(PathBuf, Result<(), WorktreeError>)> {
    find_dead_lanes(repo)
        .into_iter()
        .map(|path| {
            let result = remove_one(repo, &path);
            (path, result)
        })
        .collect()
}

fn remove_one(repo: &Path, path: &Path) -> Result<(), WorktreeError> {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
    let branch = format!("fleet/{name}");
    let worktree = fleet_merge::Worktree { path: path.to_path_buf(), branch, name };
    fleet_merge::remove(repo, &worktree)
}

#[cfg(test)]
#[path = "reap_sweep_tests.rs"]
mod tests;
