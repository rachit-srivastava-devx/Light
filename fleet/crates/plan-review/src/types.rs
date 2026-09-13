use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Input to the review gate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReviewInput {
    pub plan_digest: String,
    pub reviewer_id: String,
    pub worker_id: String,
    pub evidence_digest: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Decision {
    Accept,
    Reject,
    Revise,
}

/// A single finding produced by the reviewer.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Finding {
    pub severity: String,
    pub location: String,
    pub message: String,
}

/// Verdict produced by a PlanReviewer.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReviewVerdict {
    pub decision: Decision,
    pub findings: Vec<Finding>,
    pub input_digest: String,
    pub output_digest: String,
    pub checked: u64,
    pub total: u64,
}

/// Canonical plan proposal forwarded to the reviewer.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlanProposal {
    pub plan_digest: String,
    pub worker_id: String,
    pub evidence_digest: String,
}

/// Digest record produced after review acceptance.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReviewedPlanDigest {
    pub plan_digest: String,
    pub reviewer_id: String,
    pub approved: bool,
    pub checked: u64,
    pub total: u64,
}

/// Walkthrough message emitted before ready signal.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlanWalkthrough {
    pub plan_digest: String,
    pub reviewer_id: String,
    pub findings: Vec<Finding>,
}

/// Event emitted during a review session.
#[derive(Clone, Debug)]
pub enum ReviewEvent {
    Walkthrough(PlanWalkthrough),
    Digest(ReviewedPlanDigest),
}

#[derive(Clone, Debug, Error)]
pub enum ReviewError {
    #[error("self-review not allowed: worker_id == reviewer_id")]
    NotIndependent,
    #[error("digest mismatch")]
    DigestMismatch,
    #[error("malformed finding: {0}")]
    MalformedFinding(String),
    #[error("reviewer unavailable")]
    ReviewerUnavailable,
    #[error("invalid input: {0}")]
    InvalidInput(String),
}

/// Injected reviewer port. C9: no direct SDK import.
pub trait PlanReviewer: Send + Sync {
    fn review(&self, input: &ReviewInput) -> Result<ReviewVerdict, ReviewError>;
}
