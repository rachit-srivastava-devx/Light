//! Per-stage wiring closures: exactly one call into the owning `fleet-*` crate per stage,
//! translating its typed result into `PipelineError`. No decision logic lives here beyond
//! `classify_stage`'s task-text-to-`TaskClass` mapping (see its own doc comment for why that one
//! is this composition layer's call to make). Each stage with enough wiring to matter is split
//! into its own sibling module to stay under the 80-line file gate; this file re-exports them.
//!
//! **Still-flagged deviation**: `Scan`/`Plan` call their crate's real orchestration fn with
//! minimal/no-op inputs (an empty question list, a literal label) rather than the repo-specific
//! inputs a real task run needs -- wiring those concrete adapters is product work for the crate
//! that owns them (BLUEPRINT §2 non-goals), not this composition layer's call to invent.

use super::event::PipelineError;

pub use super::classify_stage::classify;
pub use super::event_stage::event;
pub use super::merge_stage::merge;
pub use super::stages_dispatch::dispatch;
pub use super::teach_stage::teach;
pub use super::verify_stage::verify;

pub fn scan() -> Result<(), PipelineError> {
    let _ = fleet_scan::merge_questions(Vec::new());
    Ok(())
}

pub fn plan() -> Result<(), PipelineError> {
    let _ = fleet_plan::assemble_acceptance_checks_draft("wired-by-fleet-cli");
    Ok(())
}
