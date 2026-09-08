//! Presentation glue between the pipeline loop and `print::*`. The pipeline decides WHEN a stage
//! starts/finishes; `print::renderer` decides HOW that looks. Keeps `graph.rs` free of
//! formatting so it stays under the 80-line cap and the render logic stays independently
//! testable with a fixed `Duration` instead of a live clock.

use super::stage::PipelineStage;
use crate::print::human_stream::emit;
use crate::print::render_event::{Event, Outcome};
use crate::print::style::Style;
use std::time::Instant;

pub fn started(stage: PipelineStage) -> Instant {
    emit(&Event::StageStarted { stage: stage.name().to_string() }, &Style::detect());
    Instant::now()
}

pub fn finished(stage: PipelineStage, start: Instant, ok: bool) {
    let outcome = if ok { Outcome::Pass } else { Outcome::Fail };
    let event = Event::StageFinished { stage: stage.name().to_string(), outcome, elapsed: start.elapsed() };
    emit(&event, &Style::detect());
}
