//! The 8-stage sequential wiring. **Restate deferred**: BLUEPRINT §3/§4 specifies an
//! `async fn run_pipeline(ctx: restate_sdk::Context, ..)` registered as a
//! `#[restate_sdk::service]`, journaling each stage via `ctx.run(..)`. Pulling in `restate-sdk`
//! could not be wired cleanly in one pass; per the task brief's explicit fallback, this uses
//! `pipeline::step_log::StepLog` as a simpler resumable-step-log shim instead. Swapping in the
//! real SDK later means replacing this file's body with `ctx.run(..)` closures around the same
//! `stages::*` calls -- the stage functions themselves do not change.

use super::ctx::StageCtx;
use super::event::PipelineOutcome;
use super::run_ledger;
use super::run_records::RunRecords;
use super::stage::PipelineStage;
use super::stage_loop::run_through_merge;
use super::stages;
use super::step_log::StepLog;
use std::path::Path;
use std::time::Instant;
use types::{NodeId, Role, TaskId};

/// Advance one pipeline run through every stage in order, skipping any stage the step log
/// already marked done (crash-resume) and always running `Teach` last regardless of outcome.
/// `git_backed` is false only for `fleet run --no-git`; every other caller passes `true`.
pub fn run_pipeline(
    state_dir: &Path,
    repo: &Path,
    task: TaskId,
    runtime: &route::RuntimeState,
    verify_gates: &[verify::GateSpec],
    git_backed: bool,
) -> PipelineOutcome {
    // Repo-scoped: `state_dir` is shared across every `--repo` in one invocation (per-user
    // default, never per-repo -- see `runtime::state_dir_default`), so a plain
    // `StepLog::open(state_dir, task_id)` here would let repo 2+ under the same task_id find
    // repo 1's completed step log and report every stage "resumed" without running anything.
    let log = StepLog::open_for_repo(state_dir, task.as_str(), repo);
    let ctx = StageCtx {
        state_dir,
        repo,
        task: &task,
        runtime,
        verify_gates,
        git_backed,
    };
    let mut records = RunRecords::new();
    let result = run_through_merge(&log, &ctx, &mut records);

    if log.is_done(PipelineStage::Teach) {
        super::stage_report::resumed(PipelineStage::Teach);
        records.stage(PipelineStage::Teach, "resumed", None);
    } else {
        let node = NodeId::parse("pipeline-run").expect("literal matches NodeId pattern");
        let failure = result.as_ref().err().map(|e| (records.final_stage, e));
        let started = Instant::now();
        stages::teach(state_dir, node, Role::Builder, failure);
        let _ = log.mark_done(PipelineStage::Teach);
        records.stage(PipelineStage::Teach, "pass", Some(started.elapsed()));
    }
    // Teach is an always-runs epilogue, NOT where the run got to. Overwriting `final_stage` with
    // it unconditionally destroyed the only record of where a failure happened -- so a run that
    // died in `verify` told the user `next: investigate stage 'teach'`, naming a stage that had
    // printed no output and failed nothing. On success, Teach genuinely IS the last stage.
    if result.is_ok() {
        records.final_stage = PipelineStage::Teach;
    }

    run_ledger::run_end(state_dir, &task, records.final_stage, result.is_ok());
    records.into_outcome(task, result, git_backed)
}
