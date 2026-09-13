use std::collections::HashMap;

use petgraph::algo::toposort;
use petgraph::graph::NodeIndex;

use super::module_graph_sort::TopologicalSortError;
use super::ModuleGraph;

impl ModuleGraph {
    /// Get modules in batches for parallel execution.
    /// Each batch contains modules that can be executed in parallel.
    pub fn batches(&self) -> Result<Vec<Vec<String>>, TopologicalSortError> {
        let (graph, _) = self.build_digraph();

        // Detect cycles and obtain a toposort order in one call.
        let sorted_indices =
            toposort(&graph, None).map_err(|_| TopologicalSortError::CycleDetected)?;

        // BFS-level assignment: level[node] = max(level[pred] + 1) over predecessors.
        let mut levels: HashMap<NodeIndex, usize> = HashMap::new();
        for &idx in &sorted_indices {
            let level = graph
                .neighbors_directed(idx, petgraph::Direction::Incoming)
                .map(|pred| levels.get(&pred).copied().unwrap_or(0) + 1)
                .max()
                .unwrap_or(0);
            levels.insert(idx, level);
        }

        // Group node IDs by level.
        let max_level = levels.values().copied().max().unwrap_or(0);
        let mut result: Vec<Vec<String>> = vec![Vec::new(); max_level + 1];
        for (&idx, &level) in &levels {
            result[level].push(graph[idx].clone());
        }

        // Sort within each batch for determinism.
        for batch in &mut result {
            batch.sort();
        }

        // Drop any empty buckets (shouldn't occur, but be safe).
        result.retain(|b| !b.is_empty());

        Ok(result)
    }
}
