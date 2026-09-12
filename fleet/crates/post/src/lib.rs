//! Post-merge verification — exact HEAD revalidation and gate evidence.
mod port;
mod receipt;
mod verify;

use std::path::PathBuf;
use thiserror::Error;

pub use port::GateRunner;
pub use receipt::write_verdict;
pub use verify::verify_after_merge;

/// Identity of a gate that must pass after integration.
#[derive(Debug, Clone)]
pub struct GateSpec {
    pub id: String,
}

/// Authorization to run local tests in a quarantined process.
#[derive(Debug, Clone)]
pub struct Grant {
    pub run_local_tests: bool,
}

/// Counts returned by a single gate execution.
#[derive(Debug, Clone)]
pub struct GateResult {
    pub checked: u64,
    pub total: u64,
}

/// Outcome of a post-merge verification run.
#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    Pass,
    Failed,
}

/// Errors that prevent a verdict from being reached.
#[derive(Debug, Error, PartialEq)]
pub enum PostError {
    #[error("stale: HEAD or acceptance digest mismatch")]
    Stale,
    #[error("zero coverage: empty gate set")]
    ZeroCoverage,
    #[error("gate failed")]
    GateFailed,
    #[error("environment fault: tool exited with code 3")]
    EnvironmentFault,
    #[error("receipt error: {0}")]
    Receipt(String),
}

/// Request to verify an integrated tree after merge.
#[derive(Debug, Clone)]
pub struct PostRequest {
    pub repo: PathBuf,
    pub expected_head: String,
    pub acceptance_digest: String,
    pub required_gates: Vec<GateSpec>,
    pub grant: Grant,
}

/// Verdict produced by post-merge verification.
#[derive(Debug, Clone)]
pub struct PostVerdict {
    pub status: Status,
    pub head: String,
    pub checked: u64,
    pub total: u64,
    pub evidence_digest: String,
}
