//! Port trait for external research lookups. Injected; never implemented in this crate.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A single source retrieved by the research port.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub url: String,
    pub title: String,
    /// ISO 8601 publication date.
    pub date: String,
    /// Confidence the source is current and accurate (0.0 = uncertain, 1.0 = certain).
    pub uncertainty: f32,
}

/// Errors a `ResearchPort` implementation may produce.
#[derive(Debug, Error)]
pub enum ResearchError {
    /// The external source could not be reached within the deadline.
    #[error("timeout")]
    Timeout,
    /// The source returned content that could not be interpreted.
    #[error("invalid source: {0}")]
    InvalidSource(String),
    /// The caller supplied an empty query.
    #[error("empty query")]
    EmptyQuery,
}

/// Read-only port that performs an external research lookup for a query string.
///
/// Implementations must be `Send + Sync` so they can run in async contexts.
pub trait ResearchPort: Send + Sync {
    /// Search for `query` within `deadline_ms` and return retrieved sources, or an error.
    fn search(&self, query: &str, deadline_ms: u64) -> Result<Vec<Source>, ResearchError>;
}
