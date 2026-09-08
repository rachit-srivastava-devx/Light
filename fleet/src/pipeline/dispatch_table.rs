//! The one match arm per non-`Teach` stage, factored out of `graph.rs` to stay under the
//! 80-line file gate. Exactly one `stages::*` call per arm -- no decision logic here.

use super::event::PipelineError;
use super::stage::PipelineStage;
use super::stages;
use fleet_types::TaskId;

pub fn run_one(
    stage: PipelineStage,
    runtime: &fleet_router::RuntimeState,
    task: &TaskId,
) -> Result<(), PipelineError> {
    match stage {
        PipelineStage::Event => stages::event(),
        PipelineStage::Classify => stages::classify(),
        PipelineStage::Scan => stages::scan(),
        PipelineStage::Plan => stages::plan(),
        PipelineStage::Dispatch => stages::dispatch(runtime, task),
        PipelineStage::Verify => stages::verify(),
        PipelineStage::Merge => stages::merge(1, "pipeline-demo-branch"),
        PipelineStage::Teach => unreachable!("Teach is run as the trailer, not in this loop"),
    }
}
