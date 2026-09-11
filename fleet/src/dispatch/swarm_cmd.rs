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

/// The verify-chain decision, extracted so `--then-verify`'s wiring is unit-testable without
/// having to spawn a real lane. Returns the exact `(repo, task_id)` that a `Done` swarm should
/// hand to the verify pipeline, or `None` when verify must be skipped (flag off, or lane did
/// not finish `Done`). The `repo`/`task` fields come straight from `SwarmArgs`, so verify is
/// invoked against the same targets as the swarm call -- pinned by tests below.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct VerifyPlan {
    pub repo: String,
    pub task: String,
}

pub(crate) fn verify_plan(then_verify: bool, swarm_done: bool, repo: &str, task: &str) -> Option<VerifyPlan> {
    if !then_verify || !swarm_done {
        return None;
    }
    Some(VerifyPlan { repo: repo.to_string(), task: task.to_string() })
}

pub fn swarm(state_dir: &Path, args: SwarmArgs) -> Result<(), DispatchError> {
    // Thread the CLI's already-resolved state dir into the worker EXPLICITLY, rather than
    // relying on both sides happening to read the same env var name (the gap: a future
    // config-file layer could set the CLI's `state_dir` without `FLEET_STATE_DIR` being set at
    // all, and the two would silently diverge). Setting the var here, from the value `dispatch`
    // already threaded through as `state_dir`, makes the worker's independent env read agree
    // with the CLI's resolution by construction instead of by coincidence.
    std::env::set_var(ENV_STATE_DIR, state_dir);
    // Snapshot `repo`/`task` before `args` is partially moved into `SpawnRequest`: `--then-verify`
    // needs the same two strings after the lane joins to hand the verify pipeline the same
    // targets (never a mutated or re-parsed variant -- the "one command instead of two" promise
    // relies on this being the byte-identical pair the dev passed).
    let then_verify = args.then_verify;
    let verify_repo = args.repo.clone();
    let verify_task = args.task.clone();
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
    let done = matches!(outcome, LaneOutcome::Done { .. });
    let swarm_result: Result<(), DispatchError> = match outcome {
        LaneOutcome::Done { .. } => Ok(()),
        LaneOutcome::Refused { reason } => Err(DispatchError::Refusal(reason)),
        LaneOutcome::EnvironmentFault { detail } => Err(DispatchError::EnvFault(detail)),
    };
    // `--then-verify`: `Refused`/`EnvironmentFault` short-circuits (never chain verify on a
    // non-Done lane). A `Done` lane -- even one with `changed_files: 0` -- proceeds to the
    // verify pipeline via the SAME `run_pipeline_on` `fleet run` uses, so the two paths cannot
    // drift. The final exit code becomes verify's outcome in the Done case; swarm's own error
    // in the skip case.
    match verify_plan(then_verify, done, &verify_repo, &verify_task) {
        Some(plan) => {
            println!("-- then-verify --");
            crate::dispatch::run_cmd::run_pipeline_on(state_dir, plan.repo, plan.task, false)
        }
        None => {
            if then_verify && !done {
                println!("then-verify skipped: swarm did not finish Done");
            }
            swarm_result
        }
    }
}
