//! `InMemoryPorts` -- implements `fleet-memory`'s `NearestNeighborLookup`/`LexicalSearch`/
//! `VectorSearch` ports over a caller-supplied slice of already-loaded `MemoryItem`s. This is
//! the whole "search backend" for `sow`'s memory: no daemon, no external index -- the item count
//! for one CLI's worth of lessons is small enough that a linear scan per call is the honest,
//! simplest thing that satisfies these ports.

use fleet_memory::{
    CosineSimilarity, Embedding, LexicalHit, LexicalSearch, MemoryId, MemoryItem,
    NearestNeighborLookup, RetrieveError, VectorHit, VectorSearch,
};
use std::collections::BTreeSet;

pub struct InMemoryPorts<'a> {
    pub items: &'a [MemoryItem],
}

fn words(text: &str) -> BTreeSet<String> {
    text.split_whitespace().map(str::to_lowercase).collect()
}

impl NearestNeighborLookup for InMemoryPorts<'_> {
    fn nearest(&self, embedding: &Embedding) -> Result<Option<(MemoryId, CosineSimilarity)>, RetrieveError> {
        let mut best: Option<(MemoryId, CosineSimilarity)> = None;
        for item in self.items {
            let sim = item.embedding.cosine(embedding).map_err(|e| RetrieveError(e.to_string()))?;
            if best.as_ref().map(|(_, b)| sim.get() > b.get()).unwrap_or(true) {
                best = Some((item.id.clone(), sim));
            }
        }
        Ok(best)
    }
}

impl LexicalSearch for InMemoryPorts<'_> {
    fn search(&self, query: &str, limit: usize) -> Result<Vec<LexicalHit>, RetrieveError> {
        let query_words = words(query);
        let mut scored: Vec<(MemoryId, usize)> = self
            .items
            .iter()
            .map(|it| (it.id.clone(), words(&it.text).intersection(&query_words).count()))
            .filter(|(_, n)| *n > 0)
            .collect();
        scored.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        scored.truncate(limit);
        Ok(scored.into_iter().enumerate().map(|(rank, (id, _))| LexicalHit { id, bm25_rank: rank }).collect())
    }
}

impl VectorSearch for InMemoryPorts<'_> {
    fn knn(&self, query_embedding: &Embedding, limit: usize) -> Result<Vec<VectorHit>, RetrieveError> {
        let mut scored: Vec<(MemoryId, f64)> = Vec::new();
        for item in self.items {
            let sim = item.embedding.cosine(query_embedding).map_err(|e| RetrieveError(e.to_string()))?;
            scored.push((item.id.clone(), sim.get()));
        }
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then_with(|| a.0.cmp(&b.0)));
        scored.truncate(limit);
        Ok(scored
            .into_iter()
            .enumerate()
            .map(|(rank, (id, distance))| VectorHit { id, vector_rank: rank, distance })
            .collect())
    }
}
