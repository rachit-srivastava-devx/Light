//! `fold_conventions` -- places a `ConventionSet` into a token budget using the crate's existing
//! `compact_to_budget`/`count_tokens` (never a second, ad-hoc budgeting scheme), sorted so nearer
//! docs are considered first: with candidates pre-sorted descending by precedence, first-fit
//! preferentially keeps the nearest/most-specific doc and trims the lowest-precedence one(s) when
//! the budget is tight, and every trim is recorded rather than silently dropped.

use super::fold_types::{ConventionFold, PlacedConventionDoc, TrimmedConventionDoc};
use super::types::{ConventionDoc, ConventionSet, DocKind};
use crate::compact::compact_to_budget;
use crate::error::ContextError;
use crate::tokens::TokenModel;
use crate::types::{ScoredChunk, SymbolId};
use fleet_types::Tokens;
use std::collections::BTreeMap;

pub fn fold_conventions(
    set: &ConventionSet,
    budget: Tokens,
    model: TokenModel,
) -> Result<ConventionFold, ContextError> {
    let by_id: BTreeMap<SymbolId, &ConventionDoc> = set.docs.iter().map(|d| (doc_id(d), d)).collect();
    let candidates: Vec<ScoredChunk> = set
        .docs
        .iter()
        .map(|d| ScoredChunk {
            id: doc_id(d),
            path: d.path.clone(),
            text: d.content.clone(),
            score: d.precedence as f64,
        })
        .collect();

    let slice = compact_to_budget(candidates, budget, model, None)?;

    let docs = slice
        .chunks
        .iter()
        .map(|c| {
            let src = by_id.get(&c.id);
            PlacedConventionDoc {
                path: c.path.clone(),
                kind: src.map_or(DocKind::AgentsMd, |d| d.kind),
                precedence: src.map_or(0, |d| d.precedence),
                content: c.text.clone(),
            }
        })
        .collect();
    let trimmed = slice
        .dropped
        .iter()
        .filter_map(|id| by_id.get(id))
        .map(|d| TrimmedConventionDoc {
            path: d.path.clone(),
            kind: d.kind,
            precedence: d.precedence,
        })
        .collect();

    Ok(ConventionFold {
        docs,
        tokens_used: slice.tokens_used,
        tokens_budget: slice.tokens_budget,
        trimmed,
    })
}

fn doc_id(d: &ConventionDoc) -> SymbolId {
    SymbolId::derive(&d.path, "convention", d.precedence as u64)
}
