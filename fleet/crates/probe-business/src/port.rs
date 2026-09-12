//! Port trait for business-context recall. Injected; never implemented in this crate.

use thiserror::Error;

/// Errors a `BusinessReader` implementation may produce.
#[derive(Debug, Error)]
pub enum ProbeError {
    #[error("reader unavailable: {0}")]
    Unavailable(String),
    #[error("evidence too large")]
    BudgetExceeded,
    #[error("invalid evidence")]
    InvalidEvidence,
}

/// Read-only port that recalls relevant business context for a requirement text.
///
/// Implementations must be `Send + Sync` so the probe can run in async contexts.
pub trait BusinessReader: Send + Sync {
    /// Returns recalled context for `req_text`, or an error if the reader is unreachable.
    fn recall(&self, req_text: &str) -> Result<String, ProbeError>;
}
