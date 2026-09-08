//! Reciprocal-rank fusion, ported verbatim from `memory_store.py:386`'s `rrf_score` formula.
//! See BLUEPRINT.md §3.E, §5.

use crate::ident::MemoryId;
use crate::retrieve::{LexicalHit, VectorHit};
use std::collections::BTreeMap;

/// Reciprocal-rank-fusion constant, matching `memory_store.py:27`'s `RRF_K = 60`.
pub const RRF_K: f64 = 60.0;

/// Sums `1/(RRF_K+bm25_rank) + 1/(RRF_K+vector_rank)` per id (each term `0` if that port didn't
/// return the id), mirroring `search`'s union-by-id fusion — an id in both lists is counted once.
pub fn fuse_rrf(lexical: &[LexicalHit], vector: &[VectorHit]) -> BTreeMap<MemoryId, f64> {
    let mut rrf: BTreeMap<MemoryId, f64> = BTreeMap::new();
    for hit in lexical {
        *rrf.entry(hit.id.clone()).or_insert(0.0) += 1.0 / (RRF_K + hit.bm25_rank as f64);
    }
    for hit in vector {
        *rrf.entry(hit.id.clone()).or_insert(0.0) += 1.0 / (RRF_K + hit.vector_rank as f64);
    }
    rrf
}
