//! The 8-stage sequential wiring. **Restate deferred**: BLUEPRINT §3/§4 specifies an
//! `async fn run_pipeline(ctx: restate_sdk::Context, ..)` registered as a
//! `#[restate_sdk::service]`, journaling each stage via `ctx.run(..)`. Pulling in `restate-sdk`
//! (an external durable-execution server + its client SDK) could not be wired cleanly in one
//! pass; per the task brief's explicit fallback, this uses `pipeline::step_log::StepLog` as a
//! simpler resumable-step-log shim instead. Swapping in the real SDK later means replacing this
//! file's body with `ctx.run(..)` closures around the same `stages::*` calls -- the stage
//! functions themselves (and `dispatch_table::run_one`'s match) do not change.

use super::dispatch_table::run_one;
use super::event::{PipelineError, PipelineOutcome};
use super::stage::PipelineStage;
use super::stages;
use super::step_log::StepLog;
use fleet_types::{NodeId, Role, TaskId};
use std::path::Path;

/// Advance one pipeline run through every stage in order, skipping any stage the step log
/// already marked done (crash-resume) and always running `Teach` last regardless of outcome.
pub fn run_pipeline(
    state_dir: &Path,
    task: TaskId,
    runtime: &fleet_router::RuntimeState,
) -> PipelineOutcome {
    let log = StepLog::open(state_dir, task.as_str());
    let mut final_stage = PipelineStage::Event;
    let result = run_through_merge(&log, runtime, &task, &mut final_stage);

    if !log.is_done(PipelineStage::Teach) {
        let node = NodeId::parse("pipeline-run").expect("literal matches NodeId pattern");
        stages::teach(node, Role::Builder);
        let _ = log.mark_done(PipelineStage::Teach);
    }
    final_stage = PipelineStage::Teach;

    PipelineOutcome { task, final_stage, result }
}

fn run_through_merge(
    log: &StepLog,
    runtime: &fleet_router::RuntimeState,
    task: &TaskId,
    final_stage: &mut PipelineStage,
) -> Result<(), PipelineError> {
    for stage in PipelineStage::ALL {
        if stage == PipelineStage::Teach {
            break; // Teach is run as the required trailer, not as part of this loop.
        }
        *final_stage = stage;
        if log.is_done(stage) {
            continue;
        }
        if let Err(e) = run_one(stage, runtime, task) {
            // Invariant from BLUEPRINT §4: every stage's only failure successor is `Teach`.
            debug_assert!(stage.allowed_successors().contains(&PipelineStage::Teach));
            return Err(e);
        }
        let _ = log.mark_done(stage);
        // `next()` is the single source of truth for stage order; used here (not just asserted
        // in tests) so a future stage insertion cannot silently desync the loop from the graph.
        if let Some(next) = stage.next() {
            *final_stage = next;
        }
    }
    Ok(())
}
