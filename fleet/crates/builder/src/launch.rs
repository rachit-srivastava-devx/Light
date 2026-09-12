use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{Lease, Observation};

/// Compiled context manifest passed to the worker alongside its lease.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextManifest {
    /// Human-readable module identifier being built.
    pub module: String,
    /// Content digest of the compiled context blob.
    pub context_digest: String,
}

/// Errors produced by the builder subsystem.
#[derive(Debug, Error)]
pub enum BuilderError {
    /// A changed path or an empty write set violates the lease's write scope.
    #[error("scope violation: {0}")]
    ScopeViolation(String),
    /// The fd-3 result frame was absent, empty, or malformed.
    #[error("protocol error: {0}")]
    Protocol(String),
    /// The worker generation is stale; the lease has been superseded.
    #[error("stale lease: generation {0} rejected")]
    StaleLease(u64),
    /// The worker was cancelled before completing.
    #[error("worker cancelled")]
    Cancelled,
}

/// Runs a worker for a given lease and captures its observation.
///
/// Implementors are responsible for spawning, bounding, and reaping the
/// child process; the parent stamps actor/time/resolved model separately.
pub trait WorkerLauncher: Send + Sync {
    /// Execute the worker. Returns the raw observation on success.
    fn run(
        &self,
        lease: &Lease,
        ctx: &ContextManifest,
        fd3: i32,
    ) -> Result<Observation, BuilderError>;
}
