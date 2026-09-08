//! `RealMemory` -- the real, wired-in `fleet_scan::MemoryPort` read-side adapter for `fleet
//! sow`: recalls prior refusals whose text is similar to the current requirement, via
//! `fleet_memory::retrieve`'s real fusion/scoring. The write side lives in `memory_write.rs`
//! (kept separate to hold this file under the ≤80-line rule).

use super::clock::now;
use super::embed::embed_text;
use super::ports::InMemoryPorts;
use super::store::SowMemoryStore;
use fleet_memory::{MemoryId, MemoryItem, RetrieveError, ScoreWeights};
use fleet_scan::{EnvFault, MemoryHit, MemoryPort};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub struct RealMemory {
    state_dir: PathBuf,
}

impl RealMemory {
    pub fn new(state_dir: &Path) -> Self {
        Self { state_dir: state_dir.to_path_buf() }
    }
}

impl MemoryPort for RealMemory {
    fn recall_similar(&self, query: &str, limit: usize) -> Result<Vec<MemoryHit>, EnvFault> {
        let store = SowMemoryStore::new(&self.state_dir);
        let items = store.load().map_err(|e| EnvFault::Internal(e.to_string()))?;
        if items.is_empty() {
            return Ok(Vec::new());
        }
        let map: BTreeMap<MemoryId, MemoryItem> = items.iter().cloned().map(|it| (it.id.clone(), it)).collect();
        let query_embedding = embed_text(query);
        let ports = InMemoryPorts { items: &items };
        let weights = ScoreWeights { alpha: 0.0, beta: 0.0, gamma: 1.0 };
        let ranked = fleet_memory::retrieve(query, &query_embedding, &map, now(), limit, weights, &ports, &ports)
            .map_err(|RetrieveError(e)| EnvFault::Internal(e))?;
        // `retrieve`'s RRF-fused `relevance` decides ranking/selection; the score reported
        // outward is the item's raw cosine similarity (a real magnitude `fleet-scan`'s 0.75
        // threshold can compare against -- an RRF rank score never approaches that range).
        Ok(ranked
            .into_iter()
            .filter_map(|r| {
                let item = map.get(&r.id)?;
                let cosine = item.embedding.cosine(&query_embedding).ok()?;
                Some(MemoryHit { text: item.text.clone(), score: cosine.get().max(0.0) as f32 })
            })
            .collect())
    }
}
