//! Terminal summary for `fleet run`'s human (non-`--json`) path. Per-stage progress is emitted
//! live by `pipeline::stage_report` as the run happens; this renders the one-line-per-run
//! wrap-up once the pipeline returns.

use super::style::Style;
use super::summary::{render_summary, Checks, Summary};
use crate::pipeline::event::PipelineOutcome;
use crate::pipeline::stage::PipelineStage;

/// How many stages the pipeline attempted to reach `final_stage`, inclusive. Derived from the
/// canonical `PipelineStage::ALL` order so adding a stage cannot leave this count stale.
fn stages_attempted(final_stage: PipelineStage) -> usize {
    PipelineStage::ALL.iter().position(|s| *s == final_stage).map(|i| i + 1).unwrap_or(1)
}

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
    // Real stage counts, not `1`: the pipeline reached `final_stage`, so every stage up to and
    // including it was attempted. A failure fails exactly that last one.
    let ran = stages_attempted(outcome.final_stage);
    let summary = Summary {
        unit: "stages",
        ran,
        passed: ran - usize::from(!ok),
        failed: usize::from(!ok),
        skipped: 0,
        checks: Checks::NotApplicable,
        refusals: Vec::new(),
        next_action,
    };
    eprintln!("{}", render_summary(&summary, &style));
}
