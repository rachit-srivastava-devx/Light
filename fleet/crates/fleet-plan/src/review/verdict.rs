//! Byte-faithful port of `verdict`'s branch selection (`review.sh:131-173`) EXCLUDING the
//! `ratchet.sh check`/`accept` subprocess calls, the `transition()` calls, and
//! `append_review_receipt` -- those are the composition root's job once it has this decision.

use fleet_types::LifecycleState;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerdictName {
    Accept,
    Revise,
    Reject,
}

/// What a verdict resolves to, once eligibility and the retry ceiling are accounted for.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VerdictDecision {
    Accept,
    Revise { reentered_state: &'static str },
    Reject,
    /// The retry ceiling was already hit when a `Revise` was requested (`review.sh:153-159`).
    Escalate { attempts: u32, limit: u32 },
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum VerdictRefusal {
    /// `review.sh:134` -- every verdict requires a non-empty `--reason`.
    #[error("a reason is required for every verdict")]
    ReasonMissing,
    /// `review.sh:138` -- verdict requires the worker to be at `review`.
    #[error("worker is at {0:?}; verdict requires the review state")]
    WorkerNotInReview(LifecycleState),
    /// `review.sh:160` -- today's `review.sh` only supports re-entering at `"plan"`.
    #[error("re-entering at {0:?} is not supported; only \"plan\" is")]
    ReenterStateUnsupported(String),
}

/// `reason` is validated for presence only (its content is never inspected -- `review.sh` never
/// inspects `$reason` beyond requiring it be non-empty either).
#[allow(clippy::too_many_arguments)]
pub fn verdict_decision(
    reason: &str,
    worker_state: LifecycleState,
    verdict: VerdictName,
    attempts: u32,
    limit: u32,
    reenter_state: &str,
) -> Result<VerdictDecision, VerdictRefusal> {
    if reason.is_empty() {
        return Err(VerdictRefusal::ReasonMissing);
    }
    if !matches!(worker_state, LifecycleState::Verified) {
        return Err(VerdictRefusal::WorkerNotInReview(worker_state));
    }
    match verdict {
        VerdictName::Accept => Ok(VerdictDecision::Accept),
        VerdictName::Reject => Ok(VerdictDecision::Reject),
        VerdictName::Revise => {
            if attempts >= limit {
                return Ok(VerdictDecision::Escalate { attempts, limit });
            }
            if reenter_state != "plan" {
                return Err(VerdictRefusal::ReenterStateUnsupported(reenter_state.to_string()));
            }
            Ok(VerdictDecision::Revise { reentered_state: "plan" })
        }
    }
}
