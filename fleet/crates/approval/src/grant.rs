use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Exact scope binding for one publication action.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApprovalRequest {
    pub task_id: String,
    pub action: String,
    pub resource: String,
    pub scope_hash: String,
    pub base_commit: String,
    pub artifact_id: String,
    /// Unix seconds; must be strictly greater than `now` at approval time.
    pub expires_at: u64,
}

/// Minted capability: one action/resource/content digest, one use, expiring.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApprovalGrant {
    pub approval_id: String,
    pub request: ApprovalRequest,
    pub actor: String,
    pub issued_at: u64,
}

/// All failure modes for the approval subsystem.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ApprovalError {
    #[error("required field empty: {0}")]
    Empty(String),
    #[error("grant has expired")]
    Expired,
    #[error("scope hash mismatch")]
    ScopeMismatch,
    #[error("grant already consumed")]
    Replay,
    #[error("grant not found: {0}")]
    NotFound(String),
    #[error("store failure: {0}")]
    Store(String),
}
