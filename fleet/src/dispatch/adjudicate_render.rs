//! `render` -- `fleet adjudicate`'s human/`--json` output. Human path uses `print::style` for
//! the leading `next:` line (ADHD-reader convention: the actionable line comes first, see
//! `print::summary`'s doc) and `print::human`'s `ok`/`refused` for severity styling, honouring
//! `NO_COLOR`/`--no-color` exactly like every other render path in this binary.

use crate::print::human;
use crate::print::style::{self, Style};
use fleet_judge::Verdict;

#[derive(serde::Serialize)]
struct AdjudicateReport {
    label: Option<String>,
    confidence_pct: Option<u8>,
    because: Option<String>,
    abstained: bool,
}

pub fn render(verdict: &Verdict, json: bool) {
    if json {
        crate::print::json::print_pretty(&to_report(verdict));
        return;
    }
    let style = Style::detect();
    match verdict {
        Verdict::Decided { label, confidence_pct, because } => {
            let next = format!("verdict is `{label}` ({confidence_pct}% confidence)");
            println!("{} {next}", style.paint(style::BOLD, "next:"));
            human::ok(format!("{label} -- {because}"));
        }
        Verdict::Abstain { why } => {
            let next = "the judge could not decide -- see the reason below, then re-run with \
                more context or a human review";
            println!("{} {next}", style.paint(style::BOLD, "next:"));
            human::refused(format!("judge abstained: {why}"));
        }
    }
}

fn to_report(verdict: &Verdict) -> AdjudicateReport {
    match verdict {
        Verdict::Decided { label, confidence_pct, because } => AdjudicateReport {
            label: Some(label.clone()),
            confidence_pct: Some(*confidence_pct),
            because: Some(because.clone()),
            abstained: false,
        },
        Verdict::Abstain { why } => {
            AdjudicateReport { label: None, confidence_pct: None, because: Some(why.clone()), abstained: true }
        }
    }
}
