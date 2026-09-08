//! Pure event -> text rendering. No I/O, no clock reads (`elapsed` arrives pre-measured on the
//! event), so a fixed `Event` + fixed `Style` always produces byte-identical text -- that is what
//! makes `src/tests/renderer_golden.rs` a real regression gate instead of vibes.

use super::render_event::{Event, Outcome};
use super::style::{self, Style};
use std::time::Duration;

#[cfg(test)]
#[path = "renderer_tests.rs"]
mod tests;

fn badge_code(outcome: Outcome) -> &'static str {
    // Pick the escape code by outcome, then let `Style::paint` decide whether to apply it --
    // keeps the severity-to-colour mapping in one place instead of four `match` arms each doing
    // their own conditional formatting.
    match outcome {
        Outcome::Pass => style::GREEN,
        Outcome::Fail => style::RED,
        Outcome::Skip => style::YELLOW,
        Outcome::Progress => style::CYAN,
    }
}

fn label(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Pass => "PASS",
        Outcome::Fail => "FAIL",
        Outcome::Skip => "SKIP",
        Outcome::Progress => "....",
    }
}

fn fmt_elapsed(d: Duration) -> String {
    format!("{:.2}s", d.as_secs_f64())
}

pub fn render(event: &Event, style: &Style) -> String {
    match event {
        Event::StageStarted { stage } => {
            format!("{} stage {}", style.paint(style::CYAN, "\u{25b6}"), style.paint(style::BOLD, stage))
        }
        Event::StageFinished { stage, outcome, elapsed } => {
            let badge = style.paint(badge_code(*outcome), label(*outcome));
            format!("  {badge} stage {} ({})", style.paint(style::BOLD, stage), fmt_elapsed(*elapsed))
        }
        Event::GateVerdict { id, outcome, checked, total, detail } => {
            let badge = style.paint(badge_code(*outcome), label(*outcome));
            let counts = match (checked, total) {
                (Some(c), Some(t)) => format!(" {c}/{t}"),
                _ => String::new(),
            };
            let detail = detail.as_deref().map(|d| format!(" -- {d}")).unwrap_or_default();
            format!("    {badge} gate {id}{counts}{detail}")
        }
        Event::Worker { lane, text } => {
            format!("    [{}] {text}", style.paint(style::MAGENTA, lane))
        }
        Event::Refusal { source, reason } => {
            format!("{} {source}: {reason}", style.paint(style::RED, "REFUSED"))
        }
        Event::Note { source, text } => {
            // Deliberately not `FAIL`/`REFUSED` shaped -- see the doc comment on `Event::Note`.
            format!("    {} {source}: {text}", style.paint(style::CYAN, "note:"))
        }
    }
}
