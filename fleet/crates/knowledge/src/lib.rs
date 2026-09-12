//! Knowledge — repo memory and lesson retrieval.
//! Re-exports fleet-memory for backward compat while new implementation matures.
pub use fleet_memory::*;

pub mod clock;
pub mod filter;
pub mod item;
pub mod port;
pub mod promote;

pub use clock::{Clock, SystemClock};
pub use filter::sources;
pub use item::{KnowledgeItem, Kind, Scope, SourceManifest};
pub use port::InMemoryStore;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum KnowledgeError {
    #[error("insufficient evidence for promotion")]
    InsufficientEvidence,
    #[error("store unavailable")]
    StoreUnavailable,
    #[error("invalid item: {0}")]
    InvalidItem(String),
}

pub trait KnowledgeStore: Send + Sync {
    fn list(
        &self,
        query: &str,
        scope: &Scope,
        limit: u32,
    ) -> Result<Vec<KnowledgeItem>, KnowledgeError>;

    fn put_candidate(&self, item: KnowledgeItem) -> Result<(), KnowledgeError>;
}
