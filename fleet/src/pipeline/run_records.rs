//! `RunRecords`: the mutable accumulator the stage loop writes into, converted exactly once into
//! the `PipelineOutcome` both the human and the `--json` path render. Split from `records.rs`
//! (which owns the record shapes) for the 80-line-per-file cap.

use super::event::{PipelineError, PipelineOutcome};
use super::records::{GateRecord, StageRecord};
use super::stage::PipelineStage;
use std::time::Duration;

pub struct RunRecords {
    pub final_stage: PipelineStage,
    pub classification: Option<Box<fleet_router::Decision>>,
    pub stages: Vec<StageRecord>,
    pub gates: Vec<GateRecord>,
}

impl RunRecords {
    pub fn new() -> Self {
        Self {
            final_stage: PipelineStage::Event,
            classification: None,
            stages: Vec::new(),
            gates: Vec::new(),
        }
    }

    pub fn stage(&mut self, stage: PipelineStage, outcome: &'static str, elapsed: Option<Duration>) {
        self.stages.push(StageRecord { stage, outcome, elapsed_ms: elapsed.map(|d| d.as_millis()) });
    }

    /// `refusal` is derived here, from the one `Result` that owns the truth, so the JSON's
    /// refusal text can never disagree with the process's exit code.
    pub fn into_outcome(
        self,
        task: fleet_types::TaskId,
        result: Result<(), PipelineError>,
    ) -> PipelineOutcome {
        let refusal = result.as_ref().err().map(|e| e.to_string());
        PipelineOutcome {
            task,
            final_stage: self.final_stage,
            result,
            classification: self.classification,
            stages: self.stages,
            gates: self.gates,
            refusal,
        }
    }
}
