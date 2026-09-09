//! `Teach`'s wiring, split out of `stages.rs` to stay under the 80-line file gate. Derives a
//! `Lesson` from the stage that actually failed this run -- `stage.name()` as the `check_id`, the
//! real `PipelineError`'s `Display` as the detail -- instead of unconditionally deriving the same
//! canned `GateRefused { check_id: "pipeline", .. }` lesson regardless of whether anything failed.
//! A run with no failure has nothing to teach: `teach` is a no-op then, not a fabricated lesson.
//!
//! **S2 fix**: the derived `Lesson` used to be bound to `_lesson` and dropped -- computed, never
//! taught. It is now persisted via `dispatch::memory::record_sow_refusal`, the same real,
//! already-wired `fleet-memory` write path a `sow` refusal uses (`memory/sow.json` under
//! `state_dir`), rather than inventing a second store. `lesson_text` renders every field so a
//! reader (human or a later `sow` recall) sees the whole lesson, not just the failing check's id.

use super::event::PipelineError;
use super::stage::PipelineStage;
use crate::dispatch::memory::record_sow_refusal;
use fleet_plan::{derive_lesson, Lesson, LessonSource, TaughtOutcome};
use fleet_types::{NodeId, Role};
use std::path::Path;

fn lesson_text(lesson: &Lesson) -> String {
    format!(
        "source={} affected_leaf={} risk={} trigger={} mitigation={}",
        lesson.source.as_wire_ref(),
        lesson.affected_leaf.as_str(),
        lesson.risk,
        lesson.trigger,
        lesson.mitigation,
    )
}

pub fn teach(state_dir: &Path, node: NodeId, role: Role, failure: Option<(PipelineStage, &PipelineError)>) {
    let Some((stage, err)) = failure else { return };
    let outcome = TaughtOutcome::GateRefused { check_id: stage.name(), detail: err.to_string() };
    let source = LessonSource::ThreadLessons("fleet-cli-pipeline".to_string());
    let lesson = derive_lesson(&outcome, source, node, role);
    // A lesson that fails to persist must not be swallowed silently either -- but `Teach` has no
    // failure successor of its own (BLUEPRINT §4: every stage's only failure successor is
    // `Teach`, and `Teach` is the trailer itself), so this is recorded to stderr rather than
    // returned as a `Result` nothing downstream would observe.
    if let Err(e) = record_sow_refusal(state_dir, &lesson_text(&lesson)) {
        eprintln!("fleet: teach: lesson computed but could not be persisted: {e}");
    }
}
