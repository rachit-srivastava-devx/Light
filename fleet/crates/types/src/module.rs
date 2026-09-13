//! Module-level data structures for parallel SOW/blueprint creation and worktree orchestration:
//! dependency tracking, a state machine for parallel execution, and module graphs. Split across
//! sibling `#[path]` files to stay under the 80-line-per-file convention -- `ModuleGraph`'s small
//! `impl` blocks are legal split across files since Rust allows several inherent impls per type.

use serde::{Deserialize, Serialize};

use crate::ident::TaskId;

#[path = "module_impl.rs"]
mod module_impl;
#[path = "module_graph.rs"]
mod module_graph;
#[path = "module_graph_ready.rs"]
mod module_graph_ready;
#[path = "module_graph_sort.rs"]
mod module_graph_sort;
#[path = "module_graph_batches.rs"]
mod module_graph_batches;
#[cfg(test)]
#[path = "module_tests.rs"]
mod tests;

pub use module_graph::ModuleGraph;
pub use module_graph_sort::TopologicalSortError;

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
    Executing {
        branch: String,
        worktree_path: String,
    },
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
