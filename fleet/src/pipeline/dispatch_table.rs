//! The one match arm per non-`Teach` stage, factored out of `graph.rs` to stay under the
//! 80-line file gate. Exactly one `stages::*` call per arm -- no decision logic here.

use super::ctx::StageCtx;
use super::event::{PipelineError, StageOutput};
use super::stage::PipelineStage;
use super::stages;

pub fn run_one(stage: PipelineStage, ctx: &StageCtx) -> Result<StageOutput, PipelineError> {
    match stage {
        PipelineStage::Event => stages::event(ctx.state_dir, ctx.task).map(|_| StageOutput::None),
        PipelineStage::Classify => {
            Ok(StageOutput::Classified(Box::new(stages::classify(ctx.task.as_str(), ctx.runtime))))
        }
        PipelineStage::Scan => stages::scan().map(|_| StageOutput::None),
        PipelineStage::Plan => stages::plan().map(|_| StageOutput::None),
        PipelineStage::Dispatch => stages::dispatch(ctx.runtime, ctx.task).map(|_| StageOutput::None),
        PipelineStage::Verify => {
            stages::verify(ctx.verify_gates, ctx.repo, ctx.state_dir).map(|_| StageOutput::None)
        }
        PipelineStage::Merge => stages::merge(ctx.repo).map(|_| StageOutput::None),
        PipelineStage::Teach => unreachable!("Teach is run as the trailer, not in this loop"),
    }
}
