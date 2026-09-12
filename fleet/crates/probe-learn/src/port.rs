//! Port traits and types for the probe-learn crate.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProbeError {
    #[error("scope violation: {0}")]
    ScopeViolation(String),
    #[error("no coverage")]
    NoCoverage,
    #[error("unavailable: {0}")]
    Unavailable(String),
}

#[derive(Clone, Debug)]
pub struct LessonHit {
    pub id: String,
    pub text: String,
    pub scope: String,
}

pub trait MemoryReader: Send + Sync {
    fn recall(&self, query: &str, scope: &str, limit: u32) -> Result<Vec<LessonHit>, ProbeError>;
}
