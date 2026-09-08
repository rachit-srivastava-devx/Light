//! Pure PageRank over a caller -> callee edge list. Fixed `iterations` power-iteration (never
//! "until convergence"). Mass-conserving: scores always sum to `nodes.len() as f64`.

use crate::types::SymbolId;
use std::collections::{BTreeMap, BTreeSet};

pub fn pagerank(
    nodes: &[SymbolId],
    edges: &[(SymbolId, SymbolId)],
    damping: f64,
    iterations: u32,
) -> BTreeMap<SymbolId, f64> {
    if nodes.is_empty() {
        return BTreeMap::new();
    }
    let unique_edges: BTreeSet<(SymbolId, SymbolId)> = edges.iter().cloned().collect();
    let n = nodes.len() as f64;

    let mut out_degree: BTreeMap<SymbolId, u64> = nodes.iter().map(|id| (id.clone(), 0)).collect();
    let mut incoming: BTreeMap<SymbolId, Vec<SymbolId>> =
        nodes.iter().map(|id| (id.clone(), Vec::new())).collect();
    for (from, to) in &unique_edges {
        if let Some(count) = out_degree.get_mut(from) {
            *count += 1;
        }
        if let Some(targets) = incoming.get_mut(to) {
            targets.push(from.clone());
        }
    }

    let mut scores: BTreeMap<SymbolId, f64> = nodes.iter().map(|id| (id.clone(), 1.0)).collect();
    for _ in 0..iterations {
        let dangling_mass: f64 = nodes
            .iter()
            .filter(|id| out_degree.get(*id).copied().unwrap_or(0) == 0)
            .map(|id| scores[id])
            .sum();
        let base = (1.0 - damping) + damping * dangling_mass / n;
        let mut next = BTreeMap::new();
        for id in nodes {
            let inbound: f64 = incoming[id]
                .iter()
                .map(|from| scores[from] / out_degree[from].max(1) as f64)
                .sum();
            next.insert(id.clone(), base + damping * inbound);
        }
        scores = next;
    }
    scores
}

// Unit tests for this function live in `tests/pagerank_properties.rs` (mass conservation,
// determinism, empty input) to keep this file under the 80-line gate.
