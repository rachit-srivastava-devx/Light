//! Typed environment-failure shape a probe reports instead of panicking or blocking.

use std::time::Duration;

/// Why a probe could not complete. A probe reports this instead of panicking or blocking; the
/// merge treats a faulted probe exactly like a probe that raised zero questions, and separately
/// surfaces the fault for observability (see `AssessmentReport::faults`) -- a fault is recorded,
/// never silently dropped, and never allowed to fail the other 3 probes.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum EnvFault {
    #[error("network unavailable: {0}")]
    NetworkUnavailable(String),
    #[error("required credential missing: {0}")]
    CredentialMissing(String),
    #[error("timed out after {0:?}")]
    TimedOut(Duration),
    #[error("rate limited, retry after {0:?}")]
    RateLimited(Duration),
    /// A probe panicked; the runner caught it (see `ConcurrentRunner`) and this crate converted
    /// it into data rather than letting the panic unwind past probe boundaries.
    #[error("probe panicked: {0}")]
    Internal(String),
}
