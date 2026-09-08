//! `prepare`: worktree creation, base-commit capture, and hermetic sandbox provisioning --
//! split out of `mod.rs` to keep that file under the line budget. Every failure path here tears
//! the worktree back down before returning, matching the rest of `spawn`'s cleanup discipline.

use crate::request::SpawnError;
use crate::sandbox::hermetic_env::{self, HermeticEnv};
use crate::sandbox::resolve_hermetic_provision;
use std::path::PathBuf;

pub(super) struct Prepared {
    pub worktree: fleet_merge::Worktree,
    pub base_commit: String,
    pub sandbox_root: PathBuf,
    pub hermetic: HermeticEnv,
}

pub(super) fn prepare(repo: &std::path::Path, role_name: &str) -> Result<Prepared, SpawnError> {
    let name = fleet_merge::unique_name(role_name);
    let worktree = fleet_merge::create(repo, &name).map_err(|_| SpawnError::WorktreeCreateFailed)?;
    let base_commit = match super::base_commit::read_head(&worktree.path) {
        Ok(sha) => sha,
        Err(err) => {
            let _ = fleet_merge::remove(repo, &worktree);
            return Err(err);
        }
    };
    if let Err(err) = resolve_hermetic_provision(repo, role_name) {
        let _ = fleet_merge::remove(repo, &worktree);
        return Err(SpawnError::SandboxProvisionFailed(err.to_string()));
    }
    let sandbox_root = worktree.path.join(".fleet-sandbox");
    let hermetic = hermetic_env::build(&sandbox_root);
    Ok(Prepared { worktree, base_commit, sandbox_root, hermetic })
}
