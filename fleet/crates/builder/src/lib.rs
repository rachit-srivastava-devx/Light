//! Builder agent — lease-bound worker launcher and observation validator.
mod impl_;
pub use impl_::*;

/// Verbatim contents of `assets/claude-system-prompt.md` -- the tuned system prompt appended to
/// every Fleet-driven Claude invocation via `claude --append-system-prompt`. Kept in this crate
/// because it is bundled with the adapter shape, not a caller concern.
pub const CLAUDE_SYSTEM_PROMPT: &str =
    include_str!("../assets/claude-system-prompt.md");

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
pub fn validate_observation(o: &Observation, lease: &Lease) -> Result<(), BuilderError> {
    lease::validate_observation(o, lease)
}

/// Parse an fd-3 result frame from raw bytes.
///
/// Returns `Err(BuilderError::Protocol)` when `data` is `None` or empty —
/// success requires a well-formed fd-3 frame.
pub fn parse_fd3_frame(data: Option<&[u8]>) -> Result<String, BuilderError> {
    result::parse_fd3_frame(data)
}
