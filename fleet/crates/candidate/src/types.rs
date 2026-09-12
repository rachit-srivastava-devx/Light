use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Verified failure evidence from the `verify` node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub task_type: String,
    pub signature: String,
    pub tree_digest: String,
    pub evidence_digest: String,
    pub checked: u64,
    pub total: u64,
}

/// Lifecycle status of a candidate lesson.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Status {
    Candidate,
    Validated,
}

/// A scoped, non-active lesson candidate passed to the `offline` node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateLesson {
    pub id: String,
    pub scope: String,
    pub fixture_digest: String,
    pub provenance: Vec<String>,
    pub status: Status,
}

/// Typed refusals from the candidate node.
#[derive(Debug, Error)]
pub enum CandidateError {
    #[error("store failure (code 3)")]
    Store,
    #[error("coverage gate: checked=0 or checked!=total (code 6)")]
    Coverage,
    #[error("invalid candidate fields (code 7)")]
    Invalid,
    #[error("duplicate candidate conflicts (code 8)")]
    Conflict,
}
