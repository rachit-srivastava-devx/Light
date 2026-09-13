use std::collections::HashMap;

use super::Module;

/// A module graph for dependency management and topological sorting.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleGraph {
    /// Map of module_id to Module
    pub modules: HashMap<String, Module>,
    /// Map of module_id to list of dependent module_ids (reverse dependencies)
    pub dependents: HashMap<String, Vec<String>>,
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

    /// Get the number of modules in the graph
    pub fn len(&self) -> usize {
        self.modules.len()
    }

    /// Check if the graph is empty
    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }
}
