//! Orchestration: BM25 + vector search, fused with `fuse_rrf`, compacted to a token budget.
//! No embedding backend ships in this pass (§7/embed.rs) -- an empty/no-op `VectorIndex` (e.g.
//! `NoVectorIndex`) degrades this to BM25-only, matching the fused-list semantics exactly.

use crate::bm25::TantivyIndex;
use crate::compact::{compact_to_budget, Summarizer};
use crate::embed::VectorIndex;
use crate::error::ContextError;
use crate::fuse::fuse_rrf;
use crate::tokens::TokenModel;
use crate::types::{ContextSlice, RepoMap, ScoredChunk, SymbolId};
use fleet_types::Tokens;

pub struct RetrievalQuery<'a> {
    pub task_text: &'a str,
    pub budget: Tokens,
    pub top_k_bm25: u32,
    pub top_k_vector: u32,
    pub rrf_k: f64,
    pub token_model: TokenModel,
}

/// `query_vector` is caller-supplied (already embedded, if an embedder is wired) -- `None` skips
/// vector search entirely and fuses BM25 alone.
#[allow(clippy::too_many_arguments)]
pub fn retrieve_context(
    query: &RetrievalQuery<'_>,
    repo_map: &RepoMap,
    bm25: &TantivyIndex,
    vectors: &dyn VectorIndex,
    query_vector: Option<&[f32]>,
    chunk_text: &dyn Fn(&SymbolId) -> Option<String>,
    summarizer: Option<&dyn Summarizer>,
) -> Result<ContextSlice, ContextError> {
    let bm25_hits = bm25.search(query.task_text, query.top_k_bm25)?;
    let vector_hits = match query_vector {
        Some(vector) => vectors.nearest(vector, query.top_k_vector)?,
        None => Vec::new(),
    };
    let fused = fuse_rrf(&[bm25_hits, vector_hits], query.rrf_k);

    let mut candidates = Vec::with_capacity(fused.len());
    for (id, score) in fused {
        let Some(text) = chunk_text(&id) else {
            continue;
        };
        let path = repo_map
            .symbols
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.path.clone())
            .unwrap_or_default();
        candidates.push(ScoredChunk {
            id,
            path,
            text,
            score,
        });
    }

    compact_to_budget(candidates, query.budget, query.token_model, summarizer)
}
