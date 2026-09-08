//! `spawn`/`join` -- the fd-3 protocol's parent side, wired to worktree creation and hermetic
//! provisioning.

mod child_command;
pub mod fd3;
mod interpret;
mod join_impl;
pub mod process_group;
mod util;

use crate::request::{LaneHandle, SpawnError, SpawnRequest};
use crate::sandbox::{hermetic_env, resolve_hermetic_provision};
use fleet_types::LaneId;
use util::{which_on_path, CHILD_EXE_OVERRIDE};

pub use join_impl::join;

pub fn spawn(request: SpawnRequest) -> Result<LaneHandle, SpawnError> {
    if request.task.trim().is_empty() {
        return Err(SpawnError::EmptyTask);
    }
    if let Some(binary) = request.adapter.cli_binary_name() {
        if which_on_path(binary).is_none() && std::env::var_os(CHILD_EXE_OVERRIDE).is_none() {
            return Err(SpawnError::CliNotOnPath(request.adapter, binary));
        }
    }
    let name = fleet_merge::unique_name(request.role.name());
    let worktree = fleet_merge::create(&request.repo, &name)
        .map_err(|_| SpawnError::WorktreeCreateFailed)?;

    if let Err(err) = resolve_hermetic_provision(&request.repo, request.role.name()) {
        let _ = fleet_merge::remove(&request.repo, &worktree);
        return Err(SpawnError::SandboxProvisionFailed(err.to_string()));
    }

    let sandbox_root = worktree.path.join(".fleet-sandbox");
    let hermetic = hermetic_env::build(&sandbox_root);
    let lane_id =
        LaneId::parse(name.clone()).map_err(|e| SpawnError::ProcessSpawnFailed(e.to_string()))?;

    let mut command = match child_command::build(&request, &worktree.path, &hermetic) {
        Ok(command) => command,
        Err(err) => {
            let _ = fleet_merge::remove(&request.repo, &worktree);
            return Err(err);
        }
    };

    let channel = match fd3::wire(&mut command) {
        Some(channel) => channel,
        None => {
            let _ = fleet_merge::remove(&request.repo, &worktree);
            return Err(SpawnError::Fd3ChannelUnavailable);
        }
    };
    let child = match command.spawn() {
        Ok(child) => child,
        Err(err) => {
            unsafe {
                libc::close(channel.parent_fd);
                libc::close(channel.child_fd);
            }
            let _ = fleet_merge::remove(&request.repo, &worktree);
            return Err(SpawnError::ProcessSpawnFailed(err.to_string()));
        }
    };
    unsafe {
        libc::close(channel.child_fd);
    }
    Ok(LaneHandle {
        lane_id,
        worktree_path: worktree.path.clone(),
        child,
        parent_fd: channel.parent_fd,
        repo: request.repo,
        worktree,
        sandbox_root,
        deadline: request.deadline,
    })
}
