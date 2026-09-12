use serde::{Deserialize, Serialize};

use crate::launch::BuilderError;

/// Observation produced by a completed worker run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    /// Digest of the candidate worktree at observation time.
    pub tree_digest: String,
    /// Paths written by the worker, relative to the worktree base.
    pub changed: Vec<String>,
    /// Paths written outside the declared write scope (scope violation evidence).
    pub unexpected: Vec<String>,
    /// Exit code of the worker process.
    pub exit_code: i32,
    /// Digest of the fd-3 result frame content.
    pub fd3_digest: String,
}

/// Parse an fd-3 result frame from raw bytes.
///
/// An absent or empty frame is `Err(Protocol)` — success requires a well-formed
/// result frame; an empty fd-3 is not valid proof of a completed build.
pub(crate) fn parse_fd3_frame(data: Option<&[u8]>) -> Result<String, BuilderError> {
    let bytes = data
        .filter(|b| !b.is_empty())
        .ok_or_else(|| BuilderError::Protocol(
            "fd-3 frame absent or empty; a well-formed frame is required for success"
                .to_string(),
        ))?;
    std::str::from_utf8(bytes)
        .map(str::to_string)
        .map_err(|e| BuilderError::Protocol(format!("fd-3 frame is not valid UTF-8: {e}")))
}
