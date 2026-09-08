//! `PipelineOutcome`/`PipelineError` -- the seam where every crate's own typed error is wrapped
//! into one error type so `run_pipeline` has a single `Result` to propagate (BLUEPRINT §3).

use super::stage::PipelineStage;

/// `Event`/`Scan`/`Plan` are never constructed in this pass: `stages::{event,classify,scan,plan}`
/// are pure wiring today (BLUEPRINT §2 non-goals -- no decision logic lives in `fleet-cli`) and
/// have no real failure input fed to them yet (the concrete adapters/probes that could fail are
/// each owned by another crate and deliberately not fabricated here, see `stages.rs`'s doc
/// comment). Kept in the enum because BLUEPRINT §3 names them as the seam's shape; `allow`d
/// rather than deleted so wiring a real failing input later is a one-line change, not a new
/// variant.
#[allow(dead_code)]
#[derive(Debug)]
pub enum PipelineError {
    Event(String),
    Scan(String),
    Plan(String),
    Dispatch(fleet_router::Refusal),
    Verify(String),
    Merge(fleet_merge::MergeRefusal),
    /// The step-log / durable-journal shim itself faulted (distinct from a stage's own
    /// business error). Named `Runtime` to match BLUEPRINT §3's `PipelineError::Runtime`.
    Runtime(String),
}

impl std::fmt::Display for PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for PipelineError {}

#[derive(Debug, serde::Serialize)]
pub struct PipelineOutcome {
    pub task: fleet_types::TaskId,
    pub final_stage: PipelineStage,
    #[serde(skip)]
    pub result: Result<(), PipelineError>,
}
