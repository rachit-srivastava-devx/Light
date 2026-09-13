use std::collections::HashMap;

use petgraph::algo::toposort;
use petgraph::graph::{DiGraph, NodeIndex};

use super::ModuleGraph;

/// Error types for topological sort operations
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum TopologicalSortError {
    /// A cycle was detected in the module dependencies
    #[error("cycle detected in module dependencies")]
    CycleDetected,
}

impl ModuleGraph {
    /// Build a petgraph DiGraph from the module graph.
    /// Nodes are added in sorted ID order for deterministic traversal.
    /// Returns the graph and a map from module ID to NodeIndex.
    ///
    /// `pub(crate)`, not private: `module_graph_batches.rs`'s `batches()` reuses this exact
    /// graph/toposort pairing rather than rebuilding it with its own logic.
    pub(crate) fn build_digraph(&self) -> (DiGraph<String, ()>, HashMap<String, NodeIndex>) {
        let mut graph: DiGraph<String, ()> = DiGraph::new();
        let mut id_to_idx: HashMap<String, NodeIndex> = HashMap::new();

        // Insert nodes in sorted order so DFS visits them deterministically.
        let mut ids: Vec<&String> = self.modules.keys().collect();
        ids.sort();
        for id in &ids {
            let idx = graph.add_node((*id).clone());
            id_to_idx.insert((*id).clone(), idx);
        }

        // Add directed edges: dependency → dependent (dep must precede module).
        for module in self.modules.values() {
            for dep in &module.depends_on {
                if let (Some(&dep_idx), Some(&mod_idx)) = (
                    id_to_idx.get(dep.as_str()),
                    id_to_idx.get(module.id.as_str()),
                ) {
                    graph.add_edge(dep_idx, mod_idx, ());
                }
            }
        }

        (graph, id_to_idx)
    }

    /// Perform topological sort using petgraph.
    /// Returns a vector of module IDs in dependency order.
    pub fn topological_sort(&self) -> Result<Vec<String>, TopologicalSortError> {
        let (graph, _) = self.build_digraph();

        toposort(&graph, None)
            .map(|indices| indices.iter().map(|&idx| graph[idx].clone()).collect())
            .map_err(|_| TopologicalSortError::CycleDetected)
    }
}
