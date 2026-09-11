//! Module-level data structures for parallel SOW/blueprint creation and worktree orchestration.
//!
//! This module provides the core types for representing modules in a parallel SDLC workflow,
//! including dependency tracking, state machine for parallel execution, and module graphs.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::ident::TaskId;

/// A module represents a unit of work in the parallel SDLC workflow.
/// Each module has its own SOW, blueprint, and worktree for parallel execution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Module {
    /// Unique identifier for the module
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// SOW text for this module
    pub sow_text: String,
    /// Module IDs that this module depends on
    pub depends_on: Vec<String>,
    /// Current state in the module lifecycle
    pub state: ModuleState,
}

/// States a module can be in during the parallel SDLC workflow.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ModuleState {
    /// Module is pending - not yet being processed
    Pending,
    /// Module is currently being SOWed
    Sowing { task_id: TaskId },
    /// Module has been SOWed successfully
    Sowed { sow_id: String },
    /// Module is currently being planned
    Planning { task_id: TaskId },
    /// Module has been planned successfully with a blueprint
    Planned { blueprint: Blueprint },
    /// Module is ready for parallel execution
    Ready,
    /// Module is currently executing
    Executing { branch: String, worktree_path: String },
    /// Module has executed successfully
    Executed { commit_count: usize },
    /// Module has been merged to main
    Merged { sha: String },
}

/// A blueprint represents the planned work for a module.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Blueprint {
    /// The plan content
    pub plan: String,
    /// Acceptance criteria for the module
    pub acceptance: Vec<String>,
    /// Files that will be modified
    pub affected_files: Vec<String>,
}

/// A module graph for dependency management and topological sorting.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleGraph {
    /// Map of module_id to Module
    pub modules: HashMap<String, Module>,
    /// Map of module_id to list of dependent module_ids (reverse dependencies)
    pub dependents: HashMap<String, Vec<String>>,
}

impl Module {
    /// Create a new module with pending state
    pub fn new(id: impl Into<String>, name: impl Into<String>, sow_text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            sow_text: sow_text.into(),
            depends_on: Vec::new(),
            state: ModuleState::Pending,
        }
    }

    /// Set dependencies for this module
    pub fn with_dependencies(mut self, depends_on: Vec<String>) -> Self {
        self.depends_on = depends_on;
        self
    }

    /// Check if this module is ready to be SOWed (all dependencies are Sowed)
    pub fn can_be_sowed(&self, state_map: &HashMap<String, ModuleState>) -> bool {
        self.depends_on
            .iter()
            .all(|dep_id| matches!(state_map.get(dep_id), Some(ModuleState::Sowed { .. })))
    }

    /// Check if this module is ready for execution (all dependencies are Executed or Merged)
    pub fn can_be_executed(&self, state_map: &HashMap<String, ModuleState>) -> bool {
        self.depends_on
            .iter()
            .all(|dep_id| matches!(state_map.get(dep_id), Some(ModuleState::Executed { .. } | ModuleState::Merged { .. })))
    }
}

impl Default for ModuleGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleGraph {
    /// Create a new empty module graph
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
            dependents: HashMap::new(),
        }
    }

    /// Add a module to the graph
    pub fn add_module(&mut self, module: Module) {
        let id = module.id.clone();
        self.modules.insert(id.clone(), module);

        // Update reverse dependency map
        for dep in &self.modules.get(&id).unwrap().depends_on {
            self.dependents
                .entry(dep.clone())
                .or_default()
                .push(id.clone());
        }
    }

    /// Get a module by ID
    pub fn get(&self, id: &str) -> Option<&Module> {
        self.modules.get(id)
    }

    /// Get all modules that are ready to be SOWed (all dependencies satisfied)
    pub fn ready_for_sow(&self) -> Vec<&Module> {
        let state_map: HashMap<_, _> = self.modules.iter().map(|(k, v)| (k.clone(), v.state.clone())).collect();
        self.modules
            .values()
            .filter(|m| m.state == ModuleState::Pending && m.can_be_sowed(&state_map))
            .collect()
    }

    /// Get all modules that are ready for execution
    pub fn ready_for_execution(&self) -> Vec<&Module> {
        let state_map: HashMap<_, _> = self.modules.iter().map(|(k, v)| (k.clone(), v.state.clone())).collect();
        self.modules
            .values()
            .filter(|m| m.state == ModuleState::Ready && m.can_be_executed(&state_map))
            .collect()
    }

    /// Perform topological sort using Kahn's algorithm
    /// Returns a vector of module IDs in execution order
    pub fn topological_sort(&self) -> Result<Vec<String>, TopologicalSortError> {
        // Calculate in-degree for each module
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        for module in self.modules.values() {
            in_degree.insert(module.id.clone(), module.depends_on.len());
        }

        // Start with modules that have no dependencies
        let mut queue: Vec<String> = in_degree
            .iter()
            .filter(|(_, &degree)| degree == 0)
            .map(|(id, _)| id.clone())
            .collect();

        // Sort for determinism
        queue.sort();

        let mut result = Vec::new();

        while !queue.is_empty() {
            // Take the first module (deterministic order)
            let current = queue.remove(0);
            result.push(current.clone());

            // Reduce in-degree for dependents
            if let Some(dep_list) = self.dependents.get(&current) {
                for dep in dep_list {
                    if let Some(degree) = in_degree.get_mut(dep) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push(dep.clone());
                            queue.sort(); // Keep sorted for determinism
                        }
                    }
                }
            }
        }

        // Check for cycles
        if result.len() != self.modules.len() {
            return Err(TopologicalSortError::CycleDetected);
        }

        Ok(result)
    }

    /// Get modules in batches for parallel execution
    /// Each batch contains modules that can be executed in parallel
    pub fn batches(&self) -> Result<Vec<Vec<String>>, TopologicalSortError> {
        let sorted = self.topological_sort()?;

        let mut result: Vec<Vec<String>> = Vec::new();
        let mut processed = std::collections::HashSet::new();

        for module_id in sorted {
            let module = self.modules.get(&module_id).unwrap();

            // Find the earliest batch this module can be in
            let mut earliest_batch = 0;
            for dep in &module.depends_on {
                for (batch_idx, batch) in result.iter().enumerate() {
                    if batch.contains(dep) {
                        earliest_batch = earliest_batch.max(batch_idx + 1);
                    }
                }
            }

            // Extend result if needed
            while result.len() <= earliest_batch {
                result.push(Vec::new());
            }

            result[earliest_batch].push(module_id.clone());
            processed.insert(module_id);
        }

        Ok(result)
    }

    /// Get the number of modules in the graph
    pub fn len(&self) -> usize {
        self.modules.len()
    }

    /// Check if the graph is empty
    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }
}

/// Error types for topological sort operations
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum TopologicalSortError {
    /// A cycle was detected in the module dependencies
    #[error("cycle detected in module dependencies")]
    CycleDetected,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_creation() {
        let module = Module::new("module-1", "First Module", "SOW text here");
        assert_eq!(module.id, "module-1");
        assert_eq!(module.name, "First Module");
        assert_eq!(module.sow_text, "SOW text here");
        assert_eq!(module.depends_on, Vec::<String>::new());
        assert!(matches!(module.state, ModuleState::Pending));
    }

    #[test]
    fn test_module_with_dependencies() {
        let module = Module::new("module-1", "First Module", "SOW text")
            .with_dependencies(vec!["dep-1".to_string(), "dep-2".to_string()]);
        assert_eq!(module.depends_on, vec!["dep-1", "dep-2"]);
    }

    #[test]
    fn test_module_graph_topological_sort() {
        let mut graph = ModuleGraph::new();

        // Create modules with dependencies: A -> B -> C
        graph.add_module(Module::new("C", "C", "SOW C").with_dependencies(vec!["B".to_string()]));
        graph.add_module(Module::new("B", "B", "SOW B").with_dependencies(vec!["A".to_string()]));
        graph.add_module(Module::new("A", "A", "SOW A").with_dependencies(vec![]));

        let sorted = graph.topological_sort().unwrap();
        assert_eq!(sorted, vec!["A", "B", "C"]);
    }

    #[test]
    fn test_module_graph_batches() {
        let mut graph = ModuleGraph::new();

        // Create modules: A (independent), B depends on A, C depends on A, D depends on B and C
        graph.add_module(Module::new("A", "A", "SOW A").with_dependencies(vec![]));
        graph.add_module(Module::new("B", "B", "SOW B").with_dependencies(vec!["A".to_string()]));
        graph.add_module(Module::new("C", "C", "SOW C").with_dependencies(vec!["A".to_string()]));
        graph.add_module(Module::new("D", "D", "SOW D").with_dependencies(vec!["B".to_string(), "C".to_string()]));

        let batches = graph.batches().unwrap();
        // Batch 0: A (no deps)
        // Batch 1: B, C (depend only on A)
        // Batch 2: D (depends on B and C)
        assert_eq!(batches.len(), 3);
        assert_eq!(batches[0], vec!["A"]);
        assert!(batches[1].contains(&"B".to_string()));
        assert!(batches[1].contains(&"C".to_string()));
        assert_eq!(batches[2], vec!["D"]);
    }

    #[test]
    fn test_module_graph_cycle_detection() {
        let mut graph = ModuleGraph::new();

        // Create a cycle: A -> B -> C -> A
        graph.add_module(Module::new("A", "A", "SOW A").with_dependencies(vec!["C".to_string()]));
        graph.add_module(Module::new("B", "B", "SOW B").with_dependencies(vec!["A".to_string()]));
        graph.add_module(Module::new("C", "C", "SOW C").with_dependencies(vec!["B".to_string()]));

        let result = graph.topological_sort();
        assert!(matches!(result, Err(TopologicalSortError::CycleDetected)));
    }
}
