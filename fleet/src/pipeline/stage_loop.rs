//! `run_through_merge`, split out of `graph.rs` to stay under the 80-line file gate.

use super::ctx::StageCtx;
use super::dispatch_table::run_one;
use super::event::{PipelineError, StageOutput};
use super::run_ledger;
use super::run_records::RunRecords;
use super::stage::PipelineStage;
use super::stage_report;
use super::step_log::StepLog;

pub fn run_through_merge(
    log: &StepLog,
    ctx: &StageCtx,
    out: &mut RunRecords,
) -> Result<(), PipelineError> {
    for stage in PipelineStage::ALL {
        if stage == PipelineStage::Teach {
            break; // Teach is run as the required trailer, not as part of this loop.
        }
        out.final_stage = stage;
        if log.is_done(stage) {
            // Recorded, not silently dropped: this is why a crash-resumed run reports
            // `classification: null` for a `Classify` that passed in an EARLIER invocation.
            out.stage(stage, "resumed", None);
            continue;
        }
        // Structured start/finish events, grouped per stage (S1: `fleet run` used to hang with
        // zero output; this also tells slow-but-alive apart from hung).
        let started = stage_report::started(stage);
        let result = run_one(stage, ctx, &mut out.gates);
        let elapsed = started.elapsed();
        stage_report::finished(stage, started, result.is_ok());
        out.stage(stage, if result.is_ok() { "pass" } else { "fail" }, Some(elapsed));
        match result {
            Ok(StageOutput::Classified(decision)) => out.classification = Some(decision),
            Ok(StageOutput::None) => {}
            Err(e) => {
                // Invariant from BLUEPRINT §4: every stage's only failure successor is `Teach`.
                debug_assert!(stage.allowed_successors().contains(&PipelineStage::Teach));
                run_ledger::refusal(ctx.state_dir, stage, &e);
                return Err(e);
            }
        }
        let _ = log.mark_done(stage);
        // `next()` is the single source of truth for stage order (also asserted in tests).
        if let Some(next) = stage.next() {
            out.final_stage = next;
        }
    }
    Ok(())
}
