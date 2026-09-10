//! The one match arm per non-`Teach` stage, factored out of `graph.rs` to stay under the
//! 80-line file gate. Exactly one `stages::*` call per arm -- no decision logic here.

use super::ctx::StageCtx;
use super::event::{PipelineError, StageOutput};
use super::records::GateRecord;
use super::stage::PipelineStage;
use super::stages;

/// `gates` is an out-parameter rather than part of `StageOutput` because `Verify` must publish
/// its per-gate verdicts on BOTH paths -- a failing verify returns `Err`, and that is exactly the
/// run whose gate table a caller most needs to read.
pub fn run_one(
    stage: PipelineStage,
    ctx: &StageCtx,
    gates: &mut Vec<GateRecord>,
) -> Result<StageOutput, PipelineError> {
    match stage {
        PipelineStage::Event => stages::event(ctx.state_dir, ctx.task).map(|_| StageOutput::None),
        PipelineStage::Classify => {
            Ok(StageOutput::Classified(Box::new(stages::classify(ctx.task.as_str(), ctx.runtime))))
        }
        PipelineStage::Scan => stages::scan().map(|_| StageOutput::None),
        PipelineStage::Plan => stages::plan().map(|_| StageOutput::None),
        PipelineStage::Dispatch => stages::dispatch(ctx.runtime, ctx.task).map(|_| StageOutput::None),
        PipelineStage::Verify => {
            stages::verify(ctx.verify_gates, ctx.repo, ctx.state_dir, gates).map(|_| StageOutput::None)
        }
        PipelineStage::Merge => stages::merge(ctx.repo).map(|_| StageOutput::None),
        PipelineStage::Teach => unreachable!("Teach is run as the trailer, not in this loop"),
    }
}
