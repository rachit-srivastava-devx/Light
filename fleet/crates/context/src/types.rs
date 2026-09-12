use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A source-code span with digest and byte range.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Span {
    pub source_digest: String,
    pub start: u64,
    pub end: u64,
    pub label: String,
}

/// A single retrieved evidence item.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Evidence {
    pub source_digest: String,
    pub content: String,
    pub tokens: u64,
}

/// A reference to included evidence (deduped, ranked).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EvidenceRef {
    pub source_digest: String,
    pub label: String,
    pub tokens: u64,
}

/// Input to the context compiler.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompileInput {
    pub task_digest: String,
    pub plan_digest: String,
    pub base_digest: String,
    pub mandatory: Vec<Span>,
    pub budget: u64,
    pub reserve: u64,
}

/// Compiled context manifest.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ContextManifest {
    pub digest: String,
    pub mandatory: Vec<Span>,
    pub evidence: Vec<EvidenceRef>,
    pub omitted: Vec<String>,
    pub checked: u64,
    pub total: u64,
}

/// Retrieval query sent to the Retriever port.
#[derive(Clone, Debug)]
pub struct RetrievalQuery {
    pub task_digest: String,
    pub base_digest: String,
    pub budget: u64,
}

/// Injected retrieval port — no direct model or filesystem calls.
pub trait Retriever: Send + Sync {
    fn retrieve(&self, q: &RetrievalQuery) -> Result<Vec<Evidence>, ContextError>;
    /// Returns the digest of the indexed snapshot, used for staleness check.
    fn index_digest(&self) -> &str;
}

/// Injected token counter port.
pub trait TokenCounter: Send + Sync {
    fn count(&self, text: &str) -> u64;
}

#[derive(Clone, Debug, Error)]
pub enum ContextError {
    #[error("stale base digest: expected {expected:?}, index has {actual:?}")]
    Stale { expected: String, actual: String },
    #[error("missing provenance for evidence")]
    MissingProvenance,
    #[error("budget exceeded")]
    BudgetExceeded,
    #[error("zero coverage: no evidence included")]
    ZeroCoverage,
}
