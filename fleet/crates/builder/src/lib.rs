//! Builder agent — lease-bound worker launcher and observation validator.
//! Re-exports fleet-worker for backward compat while new implementation matures.
pub use fleet_worker::*;

pub mod launch;
pub mod lease;
pub mod result;

pub use launch::{BuilderError, ContextManifest, WorkerLauncher};
pub use lease::Lease;
pub use result::Observation;

/// Validate that `o` satisfies the scope constraints in `lease`.
///
/// Returns `Err(BuilderError::ScopeViolation)` when `changed` is empty
/// (no-op stub) or any changed path falls outside `write_scope`.
pub fn validate_observation(
    o: &Observation,
    lease: &Lease,
) -> Result<(), BuilderError> {
    lease::validate_observation(o, lease)
}

/// Parse an fd-3 result frame from raw bytes.
///
/// Returns `Err(BuilderError::Protocol)` when `data` is `None` or empty —
/// success requires a well-formed fd-3 frame.
pub fn parse_fd3_frame(data: Option<&[u8]>) -> Result<String, BuilderError> {
    result::parse_fd3_frame(data)
}
