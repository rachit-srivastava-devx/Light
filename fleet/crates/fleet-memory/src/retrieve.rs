//! `retrieve` — hybrid fusion over injected lexical/vector ports via reciprocal-rank fusion.
//! See BLUEPRINT.md §3.E.

use crate::embedding::Embedding;
use crate::fusion::fuse_rrf;
use crate::ident::MemoryId;
use crate::item::{MemoryItem, Timestamp};
use crate::score::{score, Relevance, Score, ScoreWeights};
use std::collections::BTreeMap;

/// One lexical (BM25/FTS5) hit.
#[derive(Clone, Debug)]
pub struct LexicalHit {
    pub id: MemoryId,
    pub bm25_rank: usize,
}

/// One vector (kNN) hit.
#[derive(Clone, Debug)]
pub struct VectorHit {
    pub id: MemoryId,
    pub vector_rank: usize,
    pub distance: f64,
}

/// The injected lexical-search port.
pub trait LexicalSearch {
    fn search(&self, query: &str, limit: usize) -> Result<Vec<LexicalHit>, RetrieveError>;
}
/// The injected vector-search port.
pub trait VectorSearch {
    fn knn(&self, query_embedding: &Embedding, limit: usize) -> Result<Vec<VectorHit>, RetrieveError>;
}

/// Either injected port failed.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("memory retrieval port failed: {0}")]
pub struct RetrieveError(pub String);

/// One retrieved item, ranked and ready to hand to the caller.
#[derive(Clone, Debug)]
pub struct RetrievedItem {
    pub id: MemoryId,
    pub score: Score,
    pub relevance: Relevance,
}

/// Fuse lexical/vector hits via RRF into a `Relevance` per id, combine with `score`, and return
/// the top `limit` by `Score` descending, ties broken by `MemoryId` ascending.
#[allow(clippy::too_many_arguments)]
pub fn retrieve(
    query: &str,
    query_embedding: &Embedding,
    items: &BTreeMap<MemoryId, MemoryItem>,
    now: Timestamp,
    limit: usize,
    weights: ScoreWeights,
    lexical: &dyn LexicalSearch,
    vector: &dyn VectorSearch,
) -> Result<Vec<RetrievedItem>, RetrieveError> {
    let lexical_hits = lexical.search(query, limit.max(items.len()))?;
    let vector_hits = vector.knn(query_embedding, limit.max(items.len()))?;
    let rrf = fuse_rrf(&lexical_hits, &vector_hits);

    let mut ranked: Vec<RetrievedItem> = rrf
        .into_iter()
        .filter_map(|(id, fused)| {
            let item = items.get(&id)?;
            let relevance = Relevance::new(fused.clamp(0.0, 1.0)).unwrap_or(Relevance::new(1.0).unwrap());
            Some(RetrievedItem { id, score: score(item, now, relevance, weights), relevance })
        })
        .collect();

    ranked.sort_by(|a, b| b.score.get().partial_cmp(&a.score.get()).unwrap().then_with(|| a.id.cmp(&b.id)));
    ranked.truncate(limit);
    Ok(ranked)
}
