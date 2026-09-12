use crate::types::{
    EvaluationRequest, HeldOutTask, OfflineError, OfflineScore, Outcome, Recommendation,
    ValidatedLesson, Variant,
};
use crate::{pair, stats};

/// Injected port: run one task in one arm and return its outcome.
pub trait TrialRunner {
    fn run(&self, task: &HeldOutTask, variant: Variant) -> Result<Outcome, OfflineError>;
}

/// Run all paired trials and accumulate integer pass counts.
pub fn evaluate_candidate(
    runner: &impl TrialRunner,
    req: &EvaluationRequest,
) -> Result<OfflineScore, OfflineError> {
    let pairs = pair::build_pairs(req)?;
    let total = pairs.len() as u64;
    let mut baseline_pass = 0u64;
    let mut variant_pass = 0u64;
    for trial in &pairs {
        let base = runner
            .run(trial.task, Variant::Baseline)
            .map_err(|e| OfflineError::Unavailable(e.to_string()))?;
        let cand = runner
            .run(trial.task, Variant::Candidate)
            .map_err(|e| OfflineError::Unavailable(e.to_string()))?;
        if base.passed {
            baseline_pass += 1;
        }
        if cand.passed {
            variant_pass += 1;
        }
    }
    let mut score = OfflineScore {
        candidate_id: req.candidate_id.clone(),
        baseline_pass,
        variant_pass,
        checked: total,
        total,
        recommendation: Recommendation::Reject,
    };
    stats::apply_quality_criteria(&mut score)?;
    Ok(score)
}

/// Produce a `ValidatedLesson` receipt only when the recommendation is Promote.
pub fn emit_validated_lesson(score: &OfflineScore) -> Option<ValidatedLesson> {
    if matches!(score.recommendation, Recommendation::Promote) {
        Some(ValidatedLesson {
            candidate_id: score.candidate_id.clone(),
            score: score.clone(),
        })
    } else {
        None
    }
}
