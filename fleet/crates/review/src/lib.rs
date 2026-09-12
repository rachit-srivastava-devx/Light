//! Code review — model-gated review verdict.
//! Re-exports fleet-judge for backward compat while new implementation matures.
pub use fleet_judge::*;

use serde::{Deserialize, Serialize};

pub use crate::decision::assemble_reviewed_candidate;
pub use crate::findings::apply_scope_filter;

pub mod decision;
pub mod findings;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateObservation {
    pub diff: String,
    pub changed_paths: Vec<String>,
    pub worker_id: String,
    pub reviewer_id: String,
    pub tree_digest: String,
    pub plan_digest: String,
    pub acceptance_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextManifest {
    pub scope: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub severity: String,
    pub path: String,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Decision {
    Approved,
    ChangesRequested,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewResult {
    pub decision: Decision,
    pub findings: Vec<Finding>,
    pub checked: u64,
    pub total: u64,
    pub input_digest: String,
    pub output_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewedCandidate {
    pub passed: bool,
    pub result: ReviewResult,
}

pub trait FindingsProvider: Send + Sync {
    fn findings(&self, path: &str) -> Vec<Finding>;
}

#[derive(Debug, thiserror::Error)]
pub enum ReviewError {
    #[error("self-approval: worker_id == reviewer_id")]
    NotIndependent,
    #[error("empty observation: diff is empty")]
    EmptyObservation,
    #[error("malformed: {0}")]
    Malformed(String),
    #[error("tool unavailable: {0}")]
    ToolUnavailable(String),
    #[error("mismatch: {0}")]
    Mismatch(String),
    #[error("zero coverage: no files checked")]
    ZeroCoverage,
}
