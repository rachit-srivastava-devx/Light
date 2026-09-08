//! `fleet run` and the hidden `__pipeline_probe`: parse -> `pipeline::run_pipeline` -> print.
//! This is the one place the CLI layer drives the durable pipeline graph end to end.

use crate::cli::args_ops::RunArgs;
use crate::dispatch::error::DispatchError;
use crate::pipeline::run_pipeline;
use crate::print::json;
use fleet_types::TaskId;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Every candidate confirmed installed, with an unmeasured-but-generous quota window and no
/// cooldown -- the "everything is healthy" default a fresh CLI invocation has no better basis to
/// assume (real measured values are `fleet-govern`'s job, not this composition root's).
fn healthy_runtime() -> fleet_router::RuntimeState {
    let preference: Vec<&'static str> = fleet_router::ORDER.iter().map(|c| c.id).collect();
    let capable = fleet_router::ORDER.iter().map(|c| c.adapter).collect::<BTreeSet<_>>();
    let remaining = capable.iter().map(|a| (a.to_string(), Some(u64::MAX))).collect::<BTreeMap<_, _>>();
    fleet_router::RuntimeState { capable, remaining, cooldown: BTreeSet::new(), required_tokens: 0, preference }
}

pub fn run(state_dir: &Path, args: RunArgs) -> Result<(), DispatchError> {
    let task = TaskId::parse(args.task).map_err(|e| DispatchError::Refusal(e.to_string()))?;
    let outcome = run_pipeline(state_dir, task, &healthy_runtime());
    json::print_pretty(&outcome);
    match outcome.result {
        Ok(()) => Ok(()),
        Err(e) => Err(DispatchError::Refusal(e.to_string())),
    }
}

pub fn pipeline_probe(state_dir: &Path, task_id: String) -> Result<(), DispatchError> {
    let task = TaskId::parse(task_id).map_err(|e| DispatchError::Refusal(e.to_string()))?;
    let outcome = run_pipeline(state_dir, task, &healthy_runtime());
    json::print_pretty(&outcome);
    outcome.result.map_err(|e| DispatchError::Refusal(e.to_string()))
}
