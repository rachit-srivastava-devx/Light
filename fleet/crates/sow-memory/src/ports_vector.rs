//! `InMemoryPorts`'s `VectorSearch` impl -- split out of `ports.rs` for its 80-line cap.

use super::InMemoryPorts;
use knowledge::{Embedding, MemoryId, RetrieveError, VectorHit, VectorSearch};

impl VectorSearch for InMemoryPorts<'_> {
    fn knn(
        &self,
        query_embedding: &Embedding,
        limit: usize,
    ) -> Result<Vec<VectorHit>, RetrieveError> {
        let mut scored: Vec<(MemoryId, f64)> = Vec::new();
        for item in self.items {
            let sim = item
                .embedding
                .cosine(query_embedding)
                .map_err(|e| RetrieveError(e.to_string()))?;
            scored.push((item.id.clone(), sim.get()));
        }
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then_with(|| a.0.cmp(&b.0)));
        scored.truncate(limit);
        Ok(scored
            .into_iter()
            .enumerate()
            .map(|(rank, (id, distance))| VectorHit {
                id,
                vector_rank: rank,
                distance,
            })
            .collect())
    }
}
