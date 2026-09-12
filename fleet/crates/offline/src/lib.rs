mod pair;
mod runner;
mod stats;
pub mod types;

pub use runner::{emit_validated_lesson, TrialRunner};
pub use types::{
    EvaluationRequest, HeldOutTask, LessonCandidate, OfflineError, OfflineScore, Outcome,
    Recommendation, ValidatedLesson, Variant,
};

/// Evaluate a candidate lesson against held-out tasks using deterministic paired trials.
///
/// Every task is run in both the baseline and candidate arm. The seed governs trial ordering.
///
/// # Errors
/// - [`OfflineError::Coverage`] — empty task list or `checked != total`.
/// - [`OfflineError::InsufficientPairs`] — fewer tasks than `min_pairs`.
/// - [`OfflineError::PairMismatch`] — duplicate task ids with differing digests.
/// - [`OfflineError::Unavailable`] — runner returned an error.
pub fn evaluate(
    runner: &impl TrialRunner,
    req: EvaluationRequest,
) -> Result<OfflineScore, OfflineError> {
    runner::evaluate_candidate(runner, &req)
}
