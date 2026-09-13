use crate::types::{PlanProposal, ReviewError, ReviewInput, ReviewVerdict};

/// Validate the review contract: independence and input consistency.
pub fn validate(input: &ReviewInput, v: &ReviewVerdict) -> Result<(), ReviewError> {
    if input.plan_digest.is_empty() || input.reviewer_id.is_empty() || input.worker_id.is_empty() {
        return Err(ReviewError::InvalidInput("missing required fields".into()));
    }
    if input.reviewer_id == input.worker_id {
        return Err(ReviewError::NotIndependent);
    }
    if v.checked == 0 {
        return Err(ReviewError::InvalidInput("zero checked denominator".into()));
    }
    // output_digest must be nonempty
    if v.output_digest.is_empty() || v.input_digest.is_empty() {
        return Err(ReviewError::DigestMismatch);
    }
    Ok(())
}

/// Validate a plan proposal and check independence before dispatching to the reviewer.
pub fn validate_plan_proposal(
    proposal: &PlanProposal,
    reviewer_id: &str,
) -> Result<(), ReviewError> {
    if proposal.plan_digest.is_empty() {
        return Err(ReviewError::InvalidInput("empty plan_digest".into()));
    }
    if proposal.worker_id == reviewer_id {
        return Err(ReviewError::NotIndependent);
    }
    Ok(())
}
