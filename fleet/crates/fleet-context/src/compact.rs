//! `compact_to_budget` -- greedy first-fit-by-score placement into a hard token budget, with an
//! optional injected `Summarizer` for near-misses. Never truncates `text` blindly (§4).

use crate::error::ContextError;
use crate::tokens::{count_tokens, TokenModel};
use crate::types::{ContextSlice, PlacedChunk, ScoredChunk};
use fleet_types::Tokens;

/// Shrinks `text` toward `target_tokens`. The caller performs the actual model call.
pub trait Summarizer {
    fn summarize(&self, text: &str, target_tokens: u32) -> Result<String, ContextError>;
}

pub fn compact_to_budget(
    candidates: Vec<ScoredChunk>,
    budget: Tokens,
    model: TokenModel,
    summarizer: Option<&dyn Summarizer>,
) -> Result<ContextSlice, ContextError> {
    let mut used = Tokens::ZERO;
    let mut chunks = Vec::new();
    let mut dropped = Vec::new();

    for candidate in candidates {
        let tokens = Tokens::new(count_tokens(&candidate.text, model) as u64);
        let remaining = budget.checked_sub(used).unwrap_or(Tokens::ZERO);
        if tokens <= remaining {
            used = used.checked_add(tokens).unwrap_or(budget);
            chunks.push(PlacedChunk {
                id: candidate.id,
                path: candidate.path,
                text: candidate.text,
                compacted: false,
            });
            continue;
        }
        if let Some(placed) = try_summarize(summarizer, &candidate, remaining, model)? {
            used = used.checked_add(placed.0).unwrap_or(budget);
            chunks.push(PlacedChunk {
                id: candidate.id,
                path: candidate.path,
                text: placed.1,
                compacted: true,
            });
        } else {
            dropped.push(candidate.id);
        }
    }

    Ok(ContextSlice {
        chunks,
        tokens_used: used,
        tokens_budget: budget,
        dropped,
    })
}

/// `Some((tokens, text))` if the summarizer fit `remaining`; `None` if absent or it errored.
fn try_summarize(
    summarizer: Option<&dyn Summarizer>,
    candidate: &ScoredChunk,
    remaining: Tokens,
    model: TokenModel,
) -> Result<Option<(Tokens, String)>, ContextError> {
    let Some(summarizer) = summarizer else {
        return Ok(None);
    };
    let target = remaining.get() as u32;
    let Ok(text) = summarizer.summarize(&candidate.text, target) else {
        return Ok(None);
    };
    let actual = count_tokens(&text, model);
    if actual > target {
        return Err(ContextError::SummarizeOverBudget {
            requested: target,
            actual,
        });
    }
    Ok(Some((Tokens::new(actual as u64), text)))
}
