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
use super::stage::PipelineStage;
use super::stage_loop::run_through_merge;
use super::stages;
use super::step_log::StepLog;
use fleet_types::{NodeId, Role, TaskId};
use std::path::Path;

/// Advance one pipeline run through every stage in order, skipping any stage the step log
/// already marked done (crash-resume) and always running `Teach` last regardless of outcome.
pub fn run_pipeline(
    state_dir: &Path,
    repo: &Path,
    task: TaskId,
    runtime: &fleet_router::RuntimeState,
    verify_gates: &[fleet_verify::GateSpec],
) -> PipelineOutcome {
    let log = StepLog::open(state_dir, task.as_str());
    let ctx = StageCtx { state_dir, repo, task: &task, runtime, verify_gates };
    let mut final_stage = PipelineStage::Event;
    let mut classification = None;
    let result = run_through_merge(&log, &ctx, &mut final_stage, &mut classification);

    if !log.is_done(PipelineStage::Teach) {
        let node = NodeId::parse("pipeline-run").expect("literal matches NodeId pattern");
        let failure = result.as_ref().err().map(|e| (final_stage, e));
        stages::teach(state_dir, node, Role::Builder, failure);
        let _ = log.mark_done(PipelineStage::Teach);
    }
    final_stage = PipelineStage::Teach;

    run_ledger::run_end(state_dir, &task, final_stage, result.is_ok());
    PipelineOutcome { task, final_stage, result, classification }
}
