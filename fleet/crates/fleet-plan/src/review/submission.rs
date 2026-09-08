//! Byte-faithful port of `submit`'s eligibility preconditions (`review.sh:113-124`) EXCLUDING
//! the `transition()`/`append_review_receipt` side effects it also performs (`review.sh:125-127`)
//! -- this fn only judges eligibility; the caller performs the transition/receipt if `Ok`.

use fleet_types::{LifecycleState, TaskId};

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SubmissionRefusal {
    /// `review.sh:116`.
    #[error("worker and reviewer must be different agent identities")]
    SelfReview,
    /// `review.sh:119` -- worker must be at the `verify` lifecycle state to submit.
    #[error("worker is at {state:?}; submission requires the verify state")]
    OutputNotReady { state: LifecycleState },
    /// `review.sh:121` -- reviewer must be at `new` or `plan`.
    #[error("reviewer is at {state:?}; expected new or plan")]
    ReviewerNotReady { state: LifecycleState },
    /// `review.sh:124` -- the review loop for this task already used its attempt budget.
    #[error("review loop is exhausted at {attempts}/{limit} attempts")]
    Exhausted { attempts: u32, limit: u32 },
}

#[allow(clippy::too_many_arguments)]
pub fn submission_eligible(
    worker_id: &TaskId,
    reviewer_id: &TaskId,
    worker_state: LifecycleState,
    reviewer_state: LifecycleState,
    attempts: u32,
    limit: u32,
) -> Result<(), SubmissionRefusal> {
    if worker_id == reviewer_id {
        return Err(SubmissionRefusal::SelfReview);
    }
    if !matches!(worker_state, LifecycleState::Verifying) {
        return Err(SubmissionRefusal::OutputNotReady { state: worker_state });
    }
    if !matches!(reviewer_state, LifecycleState::Intake | LifecycleState::Briefed) {
        return Err(SubmissionRefusal::ReviewerNotReady { state: reviewer_state });
    }
    if attempts >= limit {
        return Err(SubmissionRefusal::Exhausted { attempts, limit });
    }
    Ok(())
}
