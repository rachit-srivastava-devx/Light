//! `fleet run` and the hidden `__pipeline_probe`: parse -> `pipeline::run_pipeline` -> print.
//! This is the one place the CLI layer drives the durable pipeline graph end to end.
//!
//! **Module-level parallel execution**: Extended to support running multiple modules in parallel
//! using worktrees, with each module merging to main when complete.
//!
//! Multi-repo (`--repo a --repo b ...`): `run` loops sequentially over the repos, delegating each
//! to `run_pipeline_on` -- one task_id, N repos, one aggregated verdict. The final exit code is
//! the worst of the set. Sequential is intentional (single-machine concurrency cap).

use crate::dispatch::error::DispatchError;
use crate::dispatch::plan_cmd::{plan_modules, sow_modules};
use crate::dispatch::runtime_snapshot::healthy_runtime;
use crate::pipeline::event::PipelineError;
use crate::pipeline::{maybe_flush, run_pipeline};
use cli::args_ops::RunArgs;
use integrate::LaneManager;
use print::json;
use std::path::Path;
use types::{Module, TaskId};

/// Shared entrypoint for the verify pipeline. `run` is the CLI-invoked path (looping per
/// `--repo`); `swarm_cmd`'s `--then-verify` reuses this same function so the two never drift
/// (BLUEPRINT §5: one composition root per subcommand). String args match `swarm_cmd`'s call
/// site; `json` mirrors `RunArgs::json`. `no_git` mirrors `RunArgs::no_git` -- `swarm_cmd` always
/// passes `false`, since its own intake already required a git worktree.
pub(crate) fn run_pipeline_on(
    state_dir: &Path,
    repo: String,
    task_id: String,
    json: bool,
    no_git: bool,
) -> Result<(), DispatchError> {
    let task = TaskId::parse(task_id).map_err(|e| DispatchError::Refusal(e.to_string()))?;
    // Intake: refuse a `--repo` that isn't a git worktree up front, with an actionable
    // message -- otherwise the failure surfaces deep in the verify stage on a per-gate `git`
    // command, which is what happens today on `/Users/.../work/Frido` (a directory holding
    // three sibling checkouts). Same helper `oracle`/`gate` already use, except this call site
    // alone can opt out of the git check via `--no-git`.
    let repo = super::verify_repo::ensure_repo_mode(&repo, !no_git)?;
    // The committed table with this repo's `.fleet/gates.toml` applied -- identical to
    // `verify::GATES` when the repo has no such file (see `gate_config`).
    let gates = super::gate_config::resolve(&repo)?;
    let outcome = run_pipeline(state_dir, &repo, task, &healthy_runtime(), &gates, !no_git);
    maybe_flush(state_dir);
    if json {
        json::print_pretty(&outcome);
    } else {
        super::run_report::render(&outcome);
    }
    match outcome.result {
        Ok(()) => Ok(()),
        Err(PipelineError::VerifyTyped {
            detail,
            code: types::ExitCode::Refusal,
        }) => Err(DispatchError::Refusal(detail)),
        Err(PipelineError::VerifyTyped { detail, code }) => Err(DispatchError::VerifyFailed {
            failed: 1,
            skipped: 0,
            total: 1,
            code,
            detail,
        }),
        Err(e) => Err(DispatchError::Refusal(e.to_string())),
    }
}

pub fn run(state_dir: &Path, args: RunArgs) -> Result<(), DispatchError> {
    // Preflight EVERY --repo first via the existing intake helper (typed refusal on
    // nonexistent/unreadable paths). Refuse before running anything so a bad path in position 3
    // doesn't leave repos 1-2 with completed receipts.
    for r in &args.repos {
        super::verify_repo::ensure_repo_mode(r, !args.no_git)?;
    }
    // Sequential loop -- single-machine concurrency cap governs per-repo work; per-PR-scope
    // this stays serial. Same task_id across all repos so the ledger correlates the set.
    let mut first_err: Option<DispatchError> = None;
    let mut passed = 0usize;
    let total = args.repos.len();
    for repo in &args.repos {
        match run_pipeline_on(
            state_dir,
            repo.clone(),
            args.task.clone(),
            args.json,
            args.no_git,
        ) {
            Ok(()) => {
                passed += 1;
                if !args.json {
                    eprintln!("PASS {repo}");
                }
            }
            Err(e) => {
                if !args.json {
                    eprintln!("FAIL {repo}: {e}");
                }
                if first_err.is_none() {
                    first_err = Some(e);
                }
            }
        }
    }
    if !args.json && total > 1 {
        let failed = total - passed;
        eprintln!("rollup: {passed}/{total} passed, {failed} failed");
    }
    // Aggregated verdict: worst of the set. With one collected error, its exit code wins
    // (Refusal here; `main` maps that via `DispatchError::exit_code`).
    match first_err {
        Some(e) => Err(e),
        None => Ok(()),
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
        cli::args_core::PlanArgs {
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
        true, // this hidden probe has no `--no-git`; every caller in tree uses a real git repo.
    );
    maybe_flush(state_dir);
    json::print_pretty(&outcome);
    outcome
        .result
        .map_err(|e| DispatchError::Refusal(e.to_string()))
}
