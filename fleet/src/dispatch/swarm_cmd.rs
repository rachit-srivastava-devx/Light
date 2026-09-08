//! `fleet swarm`: parse -> `fleet_worker::{spawn,join}` -> print. Lane execution itself
//! (worktree, fd-3, subprocess) is entirely `fleet-worker`'s; this only builds the typed
//! `SpawnRequest` from CLI args and reports the outcome (BLUEPRINT §2 non-goals).

use crate::cli::args_core::SwarmArgs;
use crate::dispatch::error::DispatchError;
use crate::print::human;
use fleet_worker::{join, spawn, CliAdapter, SpawnRequest};
use fleet_types::{Role, TaskId};
use std::path::PathBuf;
use std::time::Duration;

pub fn swarm(args: SwarmArgs) -> Result<(), DispatchError> {
    let role = Role::parse(&args.role).map_err(|e| DispatchError::Refusal(e.to_string()))?;
    let task_id = TaskId::parse(args.task.clone()).map_err(|e| DispatchError::Refusal(e.to_string()))?;
    let request = SpawnRequest {
        repo: PathBuf::from(&args.repo),
        role,
        task_id,
        adapter: CliAdapter::Freelane,
        requested_model: None,
        task: args.prompt,
        deadline: Duration::from_secs(300),
    };
    let handle = spawn(request)?;
    human::line("lane", handle.lane_id.as_str());
    let outcome = join(handle)?;
    human::line("outcome", format!("{outcome:?}"));
    Ok(())
}
