//! `fleet swarm`: parse -> `fleet_worker::{spawn,join}` -> print. Lane execution itself
//! (worktree, fd-3, subprocess) is entirely `fleet-worker`'s; this only builds the typed
//! `SpawnRequest` from CLI args and reports the outcome (BLUEPRINT §2 non-goals).

use crate::cli::args_core::SwarmArgs;
use crate::dispatch::error::DispatchError;
use crate::print::human_stream::emit;
use crate::print::render_event::Event;
use crate::print::style::Style;
use fleet_worker::{join, spawn, CliAdapter, LaneOutcome, MergePolicy, SpawnRequest};
use fleet_types::{Role, TaskId};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Env var name `fleet-worker::spawn::worker_state_dir::resolve` reads. Kept as a literal, not a
/// shared const, since crossing the crate boundary for one string would cost more than it saves;
/// `runtime::state_dir_default::ENV_STATE_DIR` documents the same name on the CLI side.
const ENV_STATE_DIR: &str = "FLEET_STATE_DIR";

#[cfg(test)]
#[path = "swarm_cmd_tests.rs"]
mod tests;

pub fn swarm(state_dir: &Path, args: SwarmArgs) -> Result<(), DispatchError> {
    // Thread the CLI's already-resolved state dir into the worker EXPLICITLY, rather than
    // relying on both sides happening to read the same env var name (the gap: a future
    // config-file layer could set the CLI's `state_dir` without `FLEET_STATE_DIR` being set at
    // all, and the two would silently diverge). Setting the var here, from the value `dispatch`
    // already threaded through as `state_dir`, makes the worker's independent env read agree
    // with the CLI's resolution by construction instead of by coincidence.
    std::env::set_var(ENV_STATE_DIR, state_dir);
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
    let policy = if args.merge { MergePolicy::OnSuccess } else { MergePolicy::Never };
    let (outcome, merge_outcome) = join(handle, policy)?;
    if let Some(m) = &merge_outcome {
        let text = format!(
            "merged: branch={} staged={} changed={} {}..{}",
            m.branch, m.staged_files, m.changed_files, m.before, m.after
        );
        emit(&Event::Worker { lane: lane.clone(), text }, &style);
    }
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
