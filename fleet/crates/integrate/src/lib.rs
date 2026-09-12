pub mod cas;
pub mod cleanup;
pub mod merge;
pub mod receipt;

// Fleet-merge backward-compat re-exports for src/ (LaneManager, Worktree, MergeRefusal, etc.)
pub use ::merge::{
    invalidate_build_cache, MergeRefusal, PrError, WorktreeError,
    Lane, LaneManager, LaneOutcome,
    check_files_changed, check_head_moved, check_stage_nonempty,
    merge_lane, MergeOutcome, pr_emit,
    create, remove, unique_name, Worktree,
};

pub use merge::{GitPort, RealGit};

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Grant {
    pub target_ref: String,
    pub candidate_digest: String,
    pub expires_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeRequest {
    pub repo: PathBuf,
    pub expected_head: String,
    pub lane_head: String,
    pub candidate_digest: String,
    pub grant: Grant,
    pub checked: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationReceipt {
    pub before: String,
    pub after: String,
    pub lane: String,
    pub candidate_digest: String,
    pub checked: u64,
    pub total: u64,
}

#[derive(Debug, Error)]
pub enum IntegrateError {
    #[error("CAS mismatch: expected {expected}, got {actual}")]
    CasMismatch { expected: String, actual: String },
    #[error("merge conflict on branch {branch}")]
    Conflict { branch: String },
    #[error("containment violation: {path}")]
    Containment { path: String },
    #[error("grant error: {msg}")]
    Grant { msg: String },
    #[error("git error: {msg}")]
    Git { msg: String },
}

pub fn integrate(
    git: &impl GitPort,
    req: MergeRequest,
) -> Result<IntegrationReceipt, IntegrateError> {
    let before = cas::cas_predicate(&req)?;
    cas::check_grant(&req)?;
    git.merge_tree(&req)?;
    let after = git.merge(&req)?;
    let r = receipt::write_receipt(&req, &before, &after);
    cleanup::cleanup_worktree(&req.repo)?;
    Ok(r)
}
