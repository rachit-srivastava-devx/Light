//! `Teach`'s wiring, split out of `stages.rs` to stay under the 80-line file gate. Derives a
//! `Lesson` from the stage that actually failed this run -- `stage.name()` as the `check_id`, the
//! real `PipelineError`'s `Display` as the detail -- instead of unconditionally deriving the same
//! canned `GateRefused { check_id: "pipeline", .. }` lesson regardless of whether anything failed.
//! A run with no failure has nothing to teach: `teach` is a no-op then, not a fabricated lesson.

use super::event::PipelineError;
use super::stage::PipelineStage;
use fleet_plan::{derive_lesson, LessonSource, TaughtOutcome};
use fleet_types::{NodeId, Role};

pub fn teach(node: NodeId, role: Role, failure: Option<(PipelineStage, &PipelineError)>) {
    let Some((stage, err)) = failure else { return };
    let outcome = TaughtOutcome::GateRefused { check_id: stage.name(), detail: err.to_string() };
    let source = LessonSource::ThreadLessons("fleet-cli-pipeline".to_string());
    let _lesson = derive_lesson(&outcome, source, node, role);
}
