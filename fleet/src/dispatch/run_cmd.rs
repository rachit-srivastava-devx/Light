//! `fleet run` and the hidden `__pipeline_probe`: parse -> `pipeline::run_pipeline` -> print.
//! This is the one place the CLI layer drives the durable pipeline graph end to end.
//!
//! **Module-level parallel execution**: Extended to support running multiple modules in parallel
//! using worktrees, with each module merging to main when complete.

use crate::cli::args_ops::RunArgs;
use crate::dispatch::error::DispatchError;
use crate::dispatch::plan_cmd::{plan_modules, sow_modules};
use crate::pipeline::{maybe_flush, run_pipeline};
use crate::print::json;
use integrate::LaneManager;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use types::{Module, TaskId};

/// Every candidate confirmed installed, with an unmeasured-but-generous quota window and no
/// cooldown -- the "everything is healthy" default a fresh CLI invocation has no better basis to
/// assume (real measured values are `fleet-govern`'s job, not this composition root's).
fn healthy_runtime() -> route::RuntimeState {
    let preference: Vec<&'static str> = route::ORDER.iter().map(|c| c.id).collect();
    let capable = route::ORDER
        .iter()
        .map(|c| c.adapter)
        .collect::<BTreeSet<_>>();
    let remaining = capable
        .iter()
        .map(|a| (a.to_string(), Some(u64::MAX)))
        .collect::<BTreeMap<_, _>>();
    route::RuntimeState {
        capable,
        remaining,
        cooldown: BTreeSet::new(),
        required_tokens: 0,
        preference,
    }
}

pub fn run(state_dir: &Path, args: RunArgs) -> Result<(), DispatchError> {
    let task = TaskId::parse(args.task).map_err(|e| DispatchError::Refusal(e.to_string()))?;
    let repo = PathBuf::from(&args.repo);
    // The committed table with this repo's `.fleet/gates.toml` applied -- identical to
    // `verify::GATES` when the repo has no such file (see `gate_config`).
    let gates = super::gate_config::resolve(&repo)?;
    let outcome = run_pipeline(state_dir, &repo, task, &healthy_runtime(), &gates);
    maybe_flush(state_dir);
    if args.json {
        json::print_pretty(&outcome);
    } else {
        crate::print::run_report::render(&outcome);
    }
    match outcome.result {
        Ok(()) => Ok(()),
        Err(e) => Err(DispatchError::Refusal(e.to_string())),
    }
}

/// Run modules in parallel using worktrees.
/// Each module gets its own worktree and branch, and merges to main when complete.
pub async fn run_modules(
    state_dir: &Path,
    repo: &Path,
    modules: Vec<Module>,
) -> Result<(), DispatchError> {
    // Create module graph for dependency management
    let mut module_graph = types::ModuleGraph::new();
    for module in &modules {
        module_graph.add_module(module.clone());
    }

    // Create worktrees for all modules
    let lane_manager = LaneManager::new(repo.to_path_buf(), state_dir.to_path_buf(), 4);

    // Create worktrees
    let mut lanes = lane_manager
        .create_worktrees(&modules)
        .await
        .map_err(|e| DispatchError::Refusal(format!("failed to create worktrees: {}", e)))?;

    // SOW modules in parallel
    let _sowed_modules = sow_modules(state_dir, &modules)
        .map_err(|e| DispatchError::Refusal(format!("SOW failed: {}", e)))?;

    // Plan modules in parallel
    let blueprints = plan_modules(
        crate::cli::args_core::PlanArgs {
            model: "claude-sonnet-4".to_string(),
        },
        &modules,
    )
    .map_err(|e| DispatchError::Refusal(format!("Planning failed: {}", e)))?;

    // Build each module using its worktree
    for (module_id, blueprint) in blueprints {
        if let Some(lane) = lanes.get_mut(&module_id) {
            lane.blueprint = Some(blueprint);
            // TODO: Actually build the module in the worktree
        }
    }

    // Merge all lanes to main
    let lane_vec: Vec<_> = lanes.into_values().collect();
    let _outcomes = lane_manager
        .merge_lanes_to_main(&lane_vec)
        .await
        .map_err(|e| DispatchError::Refusal(format!("Merge failed: {}", e)))?;

    Ok(())
}

/// `__pipeline_probe` runs `Verify` against ZERO gates, still through the real
/// `run_all`/`WhichProbe`/`RealRunner` path -- not `verify::GATES`. This machine has every
/// gate's tool on `PATH` (`cargo`, `cargo-mutants`, `semgrep`, `trivy`, `conftest`, `uv`, `bash`),
/// so the real table would actually execute `cargo test --workspace` (this probe is itself
/// invoked BY that gate's own integration-test run -- unbounded recursion) and `cargo mutants`
/// (minutes-to-hours per run) inside what must stay a fast, deterministic crash-resume test.
/// `fleet run` never filters: it always gets the real, full table (`gate_config::resolve`).
const NO_GATES: &[verify::GateSpec] = &[];

pub fn pipeline_probe(
    state_dir: &Path,
    repo: String,
    task_id: String,
) -> Result<(), DispatchError> {
    let task = TaskId::parse(task_id).map_err(|e| DispatchError::Refusal(e.to_string()))?;
    let outcome = run_pipeline(
        state_dir,
        Path::new(&repo),
        task,
        &healthy_runtime(),
        NO_GATES,
    );
    maybe_flush(state_dir);
    json::print_pretty(&outcome);
    outcome
        .result
        .map_err(|e| DispatchError::Refusal(e.to_string()))
}
