//! Presentation glue for `verify_runner_bounded` -- structured events instead of ad hoc
//! `eprintln!`s, so a gate's running/timeout/refusal lines render through `print::renderer` like
//! every other structured line (grouped, attributed, severity-distinct) instead of a flat,
//! equal-weight trickle.
//!
//! `timed_out`/`budget_spent` render as `Event::Note`, not `GateVerdict`/`Refusal`: they report on
//! one subprocess invocation, not on the gate as a whole. The gate's own registry-id verdict
//! (`FAIL gate unit tests -- ...`) comes later from `print::verify_report`, once per gate. Giving
//! this process-level narration the same `FAIL`/`REFUSED` shape as that verdict is what produced
//! the "`cargo` fails, then `cargo` is refused" duplication this file used to have.

use crate::print::human_stream::emit;
use crate::print::render_event::{Event, Outcome};
use crate::print::style::Style;
use std::time::Duration;

/// Script gates resolve to an absolute materialized-tempdir path (e.g.
/// `/var/folders/.../policy/run.sh`). Showing only the file name drops the one thing that tells
/// two script gates apart when they share a script basename (`policy/run.sh` vs. `corpus/run.sh`
/// both being just `run.sh`), so this keeps one extra path segment -- the gate's own script
/// subdirectory -- whenever the command is a real path rather than a bare tool name like `cargo`.
fn display_name(bin: &str) -> String {
    let path = std::path::Path::new(bin);
    let file = path.file_name().and_then(|n| n.to_str()).unwrap_or(bin);
    let parent = path.parent().and_then(|d| d.file_name()).and_then(|n| n.to_str());
    match parent {
        // Gates are materialised into a fresh tempdir, so the parent is a random `.tmpXXXXXX` that
        // means nothing to a reader -- it leaked as `.tmpwRfP6f/semgrep-gate.sh`. Keep a REAL parent
        // (`policy/run.sh` vs `corpus/run.sh` genuinely disambiguates two gates sharing a basename).
        Some(dir) if dir.starts_with(".tmp") => file.to_string(),
        Some(dir) if path.components().count() > 1 => format!("{dir}/{file}"),
        _ => file.to_string(),
    }
}

pub fn running(command: &str, budget: Duration) {
    let event = Event::GateVerdict {
        id: command.to_string(),
        outcome: Outcome::Progress,
        checked: None,
        total: None,
        detail: Some(format!("running (budget {budget:?})")),
    };
    emit(&event, &Style::detect());
}

pub fn budget_spent(bin: &str) {
    let event = Event::Note { source: display_name(bin), text: "verify budget already spent".into() };
    emit(&event, &Style::detect());
}

pub fn timed_out(bin: &str) {
    let event = Event::Note { source: display_name(bin), text: "timed out, killed".into() };
    emit(&event, &Style::detect());
}

#[cfg(test)]
#[path = "verify_report_tests.rs"]
mod tests;
