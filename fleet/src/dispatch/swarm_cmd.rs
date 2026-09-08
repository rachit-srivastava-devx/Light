//! `fleet swarm`: parse -> `fleet_worker::{spawn,join}` -> print. Lane execution itself
//! (worktree, fd-3, subprocess) is entirely `fleet-worker`'s; this only builds the typed
//! `SpawnRequest` from CLI args and reports the outcome (BLUEPRINT §2 non-goals).

use crate::cli::args_core::SwarmArgs;
use crate::dispatch::error::DispatchError;
use crate::print::human_stream::emit;
use crate::print::render_event::Event;
use crate::print::style::Style;
use fleet_worker::{join, spawn, CliAdapter, LaneOutcome, SpawnRequest};
use fleet_types::{Role, TaskId};
use std::path::PathBuf;
use std::time::Duration;

pub fn swarm(args: SwarmArgs) -> Result<(), DispatchError> {
    let role = Role::parse(&args.role).map_err(|e| DispatchError::Refusal(e.to_string()))?;
    let task_id = TaskId::parse(args.task.clone()).map_err(|e| DispatchError::Refusal(e.to_string()))?;
    // `--task` is both the lane's task id and, unless `--prompt` overrides it, the free-text
    // instructions sent to the worker (`SpawnRequest::task`) -- this used to be wired to
    // `args.prompt` alone, so a non-empty `--task` with no `--prompt` was rejected as an empty
    // prompt (S1-4). `--prompt` remains a genuinely distinct, optional override: pass it to
    // give the worker different instructions than the task id/name itself.
    let prompt = if args.prompt.trim().is_empty() { args.task.clone() } else { args.prompt };
    let request = SpawnRequest {
        repo: PathBuf::from(&args.repo),
        role,
        task_id,
        adapter: CliAdapter::Freelane,
        requested_model: None,
        task: prompt,
        deadline: Duration::from_secs(300),
    };
    let handle = spawn(request)?;
    let lane = handle.lane_id.as_str().to_string();
    let style = Style::detect();
    // Lane-attributed lines -- so this worker's output is never blurred with any other lane's.
    emit(&Event::Worker { lane: lane.clone(), text: "spawned".into() }, &style);
    let outcome = join(handle)?;
    emit(&Event::Worker { lane, text: format!("outcome: {outcome:?}") }, &style);
    // The EXIT CODE must agree with the receipt. `swarm` used to exit 0 for every outcome, so a
    // lane that changed nothing ("the adapter returned advice", `changed_files: 0`) still looked
    // like success to a script or CI -- an honest label paired with a lying exit code.
    match outcome {
        LaneOutcome::Done { .. } => Ok(()),
        LaneOutcome::Refused { reason } => Err(DispatchError::Refusal(reason)),
        LaneOutcome::EnvironmentFault { detail } => Err(DispatchError::EnvFault(detail)),
    }
}
