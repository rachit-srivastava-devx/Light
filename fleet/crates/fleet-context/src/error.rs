//! `ContextError` -- every fallible op in this crate returns this, never `String`/`anyhow`.

use crate::types::Language;

#[derive(Debug, thiserror::Error)]
pub enum ContextError {
    #[error("tree-sitter grammar failed to load for {0:?}")]
    GrammarLoad(Language),
    #[error("source for {path:?} contains a parse error tree-sitter could not recover from")]
    ParseError { path: String },
    #[error("tantivy index operation failed: {0}")]
    Bm25Index(String),
    #[error("embedding backend failed: {0}")]
    Embed(String),
    #[error("summarizer returned {actual} tokens, over the {requested} token target")]
    SummarizeOverBudget { requested: u32, actual: u32 },
    #[error(
        "budget of {budget} tokens cannot fit even the single smallest candidate ({smallest} tokens)"
    )]
    BudgetTooSmall { budget: u64, smallest: u64 },
}
