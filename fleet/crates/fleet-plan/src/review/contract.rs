//! Byte-faithful port of `validate_review_contract` (`review.sh:45-55`) -- the two role-JSON
//! reads it performs today (`role_json "$worker_role"`/`role_json "$reviewer_role"`) are the
//! caller's job; both `RoleContract` values arrive already resolved.

use std::collections::BTreeSet;

/// The declared entity shape `review.sh` fetches per-role via `roles.sh show ROLE`
/// (`review.sh:37-43`) -- here a plain caller-supplied value, never fetched by this crate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoleContract {
    pub reviewer: String,
    pub may_produce: BTreeSet<String>,
    pub must_consume: BTreeSet<String>,
    pub cycle_states: BTreeSet<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ReviewContractViolation {
    /// `review.sh:50-52` -- the worker's declared `reviewer` field names a different role.
    #[error("role {worker_role:?} must be reviewed by {expected:?}, not {reviewer_role:?}")]
    WrongAuthority { worker_role: String, expected: String, reviewer_role: String },
    /// `review.sh:53`.
    #[error("role {0:?} cannot produce work_output")]
    RoleCannotProduce(String),
    /// `review.sh:54`.
    #[error("reviewer role {0:?} cannot consume work_output through the review cycle state")]
    ReviewerCycleMissing(String),
}

pub fn validate_review_contract(
    worker_role: &str,
    worker: &RoleContract,
    reviewer_role: &str,
    reviewer: &RoleContract,
) -> Result<(), ReviewContractViolation> {
    if worker.reviewer != reviewer_role {
        return Err(ReviewContractViolation::WrongAuthority {
            worker_role: worker_role.to_string(),
            expected: worker.reviewer.clone(),
            reviewer_role: reviewer_role.to_string(),
        });
    }
    if !worker.may_produce.contains("work_output") {
        return Err(ReviewContractViolation::RoleCannotProduce(worker_role.to_string()));
    }
    if !reviewer.must_consume.contains("work_output") || !reviewer.cycle_states.contains("review") {
        return Err(ReviewContractViolation::ReviewerCycleMissing(reviewer_role.to_string()));
    }
    Ok(())
}
