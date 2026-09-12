//! Review verdict decision -- fleet/registry-reference/registry/services/review/review.sh

mod contract;
mod submission;
mod verdict;

pub use contract::{validate_review_contract, ReviewContractViolation, RoleContract};
pub use submission::{submission_eligible, SubmissionRefusal};
pub use verdict::{verdict_decision, VerdictDecision, VerdictName, VerdictRefusal};
