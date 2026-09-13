use crate::table::CandidateSpec;
use crate::types::{Refusal, RuntimeState};

/// Stage 4: drop candidates whose adapter is cooling down or lacks enough measured quota. An
/// unmeasured (`None`) window is never treated as available.
pub(crate) fn filter_quota(candidates: &mut Vec<CandidateSpec>, runtime: &RuntimeState) {
    candidates.retain(|c| {
        !runtime.cooldown.contains(c.adapter)
            && runtime
                .remaining
                .get(c.adapter)
                .copied()
                .flatten()
                .is_some_and(|r| r >= runtime.required_tokens)
    });
}

/// Stage 4's refusal, if `candidates` came back empty.
pub(crate) fn refusal_quota(
    candidates: &[CandidateSpec],
    runtime: &RuntimeState,
) -> Option<Refusal> {
    candidates.is_empty().then(|| Refusal {
        stage: 4,
        stage_name: "availability/quota",
        reason: "all eligible lanes are cooling down or lack a sufficient known quota window"
            .into(),
        fix: format!(
            "wait for cooldown/reset or configure a measured window of at least {} tokens",
            runtime.required_tokens
        ),
    })
}
