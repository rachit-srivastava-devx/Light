//! The terminal summary block: what ran, what passed/failed/skipped (with published
//! denominators), what was refused and why, and -- first, not last -- the actionable next step.
//! ADHD-reader requirement from the brief: lead with what to do, keep it short, never bury the
//! outcome under narration.
//!
//! `gates` and `checks` are DIFFERENT units and must never be printed as one fraction: `gates` is
//! how many gates were attempted (a count of gates), `checks` is how many individual checks a
//! gate's own internal invariant actually examined (a gate-published denominator, summed across
//! every gate that published one). A run where every gate times out attempts 8 gates and performs
//! 0 checks -- both numbers are true, in different units, and conflating them into `checked 0/8`
//! reads as "nothing was checked" and "8 things were checked" at once.

use super::style::{self, Style};

#[cfg(test)]
#[path = "summary_tests.rs"]
mod tests;

/// The aggregate check-count line, when one applies. `NotApplicable` is for summaries that never
/// had gate-published counts to begin with (e.g. `fleet run`'s single-stage wrap-up) -- printing
/// `checks 0 performed` there would be as misleading as the bug this type exists to fix.
pub enum Checks {
    NotApplicable,
    Performed { checked: u64, total: u64 },
}

pub struct Summary {
    /// What `ran`/`passed`/`failed` COUNT. `fleet run` counts stages; `fleet gate`/`oracle` count
    /// gates. Hardcoding "gates" printed `gates 1 attempted -- 0 passed, 1 failed` directly under
    /// six passing gates, because `fleet run` was summarising its pipeline as one unit through a
    /// label that named a different one -- the same unit conflation this module's header warns
    /// about for gates-vs-checks, in the line that does the warning.
    pub unit: &'static str,
    pub ran: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub checks: Checks,
    pub refusals: Vec<(String, String)>,
    pub next_action: Option<String>,
}

fn checks_line(checks: &Checks, style: &Style) -> Option<String> {
    match checks {
        Checks::NotApplicable => None,
        Checks::Performed { checked: 0, .. } => Some(format!(
            "{}  (no gate examined any input -- treat this run as a failure, not a pass)",
            style.paint(style::RED, "checks   0 performed")
        )),
        Checks::Performed { checked, total } => Some(format!("checks   {checked}/{total} performed")),
    }
}

pub fn render_summary(s: &Summary, style: &Style) -> String {
    let mut lines = Vec::new();
    if let Some(next) = &s.next_action {
        lines.push(format!("{} {next}", style.paint(style::BOLD, "next:")));
    }
    lines.push(style.paint(style::BOLD, "-- summary --"));
    lines.push(format!(
        "{:<8} {} attempted -- {} passed, {} failed, {} skipped",
        s.unit, s.ran, s.passed, s.failed, s.skipped
    ));
    if let Some(line) = checks_line(&s.checks, style) {
        lines.push(line);
    }
    for (source, reason) in &s.refusals {
        lines.push(format!("{} {source}: {reason}", style.paint(style::RED, "refused")));
    }
    lines.join("\n")
}
