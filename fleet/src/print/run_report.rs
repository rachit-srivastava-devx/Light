//! Terminal summary for `fleet run`'s human (non-`--json`) path. Per-stage progress is emitted
//! live by `pipeline::stage_report` as the run happens; this renders the one-line-per-run
//! wrap-up once the pipeline returns.

use super::style::Style;
use super::summary::{render_summary, Checks, Summary};
use crate::pipeline::event::PipelineOutcome;

pub fn render(outcome: &PipelineOutcome) {
    let style = Style::detect();
    let ok = outcome.result.is_ok();
    let next_action = if ok {
        None
    } else {
        let stage = outcome.final_stage.name();
        let err = outcome.result.as_ref().err().map(|e| e.to_string()).unwrap_or_default();
        Some(format!("investigate stage `{stage}`: {err}"))
    };
    let summary = Summary {
        ran: 1,
        passed: usize::from(ok),
        failed: usize::from(!ok),
        skipped: 0,
        checks: Checks::NotApplicable,
        refusals: Vec::new(),
        next_action,
    };
    eprintln!("{}", render_summary(&summary, &style));
}
