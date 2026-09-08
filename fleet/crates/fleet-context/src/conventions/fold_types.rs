//! Result types for `fold_conventions`.

use super::types::DocKind;
use fleet_types::Tokens;

pub struct PlacedConventionDoc {
    pub path: String,
    pub kind: DocKind,
    pub precedence: u32,
    pub content: String,
}

pub struct TrimmedConventionDoc {
    pub path: String,
    pub kind: DocKind,
    pub precedence: u32,
}

pub struct ConventionFold {
    pub docs: Vec<PlacedConventionDoc>,
    pub tokens_used: Tokens,
    pub tokens_budget: Tokens,
    pub trimmed: Vec<TrimmedConventionDoc>,
}
