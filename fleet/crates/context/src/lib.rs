//! Context compiler — assemble token-budgeted context from store records.

mod impl_;
pub use impl_::*;

pub mod budget;
pub mod compile;
pub mod manifest;
pub mod types;

pub use compile::compile;
pub use types::{
    CompileInput, ContextError, ContextManifest, Evidence, EvidenceRef,
    RetrievalQuery, Retriever, Span, TokenCounter,
};
