//! Presentation glue between the pipeline loop and `print::*`. The pipeline decides WHEN a stage
//! starts/finishes; `print::renderer` decides HOW that looks. Keeps `graph.rs` free of
//! formatting so it stays under the 80-line cap and the render logic stays independently
//! testable with a fixed `Duration` instead of a live clock.

use super::stage::PipelineStage;
use print::human_stream::emit;
use print::render_event::{Event, Outcome};
use print::style::Style;
use std::time::Instant;

pub fn started(stage: PipelineStage) -> Instant {
    emit(
        &Event::StageStarted {
            stage: stage.name().to_string(),
            lld_node: stage.lld_nodes().to_string(),
        },
        &Style::detect(),
    );
    Instant::now()
}

pub fn finished(stage: PipelineStage, start: Instant, outcome: Outcome) {
    let event = Event::StageFinished {
        stage: stage.name().to_string(),
        outcome,
        elapsed: start.elapsed(),
    };
    emit(&event, &Style::detect());
}
