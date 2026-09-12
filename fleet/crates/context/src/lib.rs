//! Context compiler — assemble token-budgeted context from store records.
//! Re-exports fleet-context for backward compat while new implementation matures.
pub use fleet_context::*;

pub mod budget;
pub mod compile;
pub mod manifest;
pub mod types;

pub use compile::compile;
pub use types::{
    CompileInput, ContextError, ContextManifest, Evidence, EvidenceRef,
    RetrievalQuery, Retriever, Span, TokenCounter,
};
