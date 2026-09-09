//! Renders a `fleet_verify::Report` (from `fleet oracle`/`fleet gate`) as structured events plus
//! a terminal summary. Lives in `print/`, not `dispatch/`, because it only decides what a
//! `Report` LOOKS like -- `Report::exit_code()` still owns pass/fail, called by `dispatch`.

use super::human_stream::emit;
use super::render_event::{Event, Outcome};
use super::style::Style;
use super::summary::{render_summary, Checks, Summary};
use fleet_verify::{GateResult, Report, Verdict};

/// The gate-published `(checked, total)` denominator, when this verdict carries one -- shared by
/// `line_for` (per-gate display) and `aggregate_checks` (the summary's aggregate `checks` line)
/// so the two never drift into disagreeing about what counts as "this gate published a count".
fn denominator(v: &Verdict) -> (Option<u64>, Option<u64>) {
    match v {
        Verdict::Pass(d) => (Some(d.numerator()), Some(d.total())),
        Verdict::Fail { denominator: Some(d), .. } => (Some(d.numerator()), Some(d.total())),
        Verdict::Fail { denominator: None, .. } | Verdict::Skip { .. } => (None, None),
    }
}

/// `pub(crate)`, not `pub`: `pipeline::verify_stage` composes this same event construction for
/// its own per-gate ledger receipts, so `fleet run`'s live gate lines and its durable trail can
/// never drift into disagreeing about what one `GateResult` means (one source of truth, two
/// consumers -- see the pipeline-wiring brief this shares a call site with).
pub(crate) fn line_for(r: &GateResult) -> Event {
    let (checked, total) = denominator(&r.verdict);
    let (outcome, detail) = match &r.verdict {
        Verdict::Pass(_) => (Outcome::Pass, None),
        Verdict::Fail { reason, .. } => (Outcome::Fail, Some(format!("{reason:?}"))),
        Verdict::Skip { reason, .. } => (Outcome::Skip, Some(reason.clone())),
    };
    Event::GateVerdict { id: r.id.to_string(), outcome, checked, total, detail }
}

/// Sum every gate's published `(checked, total)` into one aggregate -- NOT the same quantity as
/// `passed`/`ran` above, which count gates, not the checks inside them. A gate with no published
/// denominator (skipped, or failed before it could examine anything) contributes 0/0.
fn aggregate_checks(report: &Report) -> Checks {
    let (mut checked_sum, mut total_sum) = (0u64, 0u64);
    for r in &report.results {
        let (checked, total) = denominator(&r.verdict);
        checked_sum += checked.unwrap_or(0);
        total_sum += total.unwrap_or(0);
    }
    Checks::Performed { checked: checked_sum, total: total_sum }
}

/// Lead with the actionable thing: the first failing gate's id, so the reader's next command is
/// the first line they see, not buried under a recap of every gate that already passed.
fn next_action(report: &Report) -> Option<String> {
    report
        .results
        .iter()
        .find(|r| matches!(r.verdict, Verdict::Fail { .. }))
        .map(|r| format!("fix and re-run: fleet gate --id \"{}\"", r.id))
}

pub fn render(report: &Report) {
    let style = Style::detect();
    for r in &report.results {
        emit(&line_for(r), &style);
    }
    let summary = Summary {
        unit: "gates",
        ran: report.results.len(),
        passed: report.passed(),
        failed: report.failed(),
        skipped: report.skipped(),
        checks: aggregate_checks(report),
        refusals: Vec::new(),
        next_action: next_action(report),
    };
    eprintln!("{}", render_summary(&summary, &style));
}
