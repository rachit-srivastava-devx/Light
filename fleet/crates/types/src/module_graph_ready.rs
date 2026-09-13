use std::collections::HashMap;

use super::{Module, ModuleGraph, ModuleState};

impl ModuleGraph {
    /// Get all modules that are ready to be SOWed (all dependencies satisfied)
    pub fn ready_for_sow(&self) -> Vec<&Module> {
        let state_map: HashMap<_, _> = self
            .modules
            .iter()
            .map(|(k, v)| (k.clone(), v.state.clone()))
            .collect();
        self.modules
            .values()
            .filter(|m| m.state == ModuleState::Pending && m.can_be_sowed(&state_map))
            .collect()
    }

    /// Get all modules that are ready for execution
    pub fn ready_for_execution(&self) -> Vec<&Module> {
        let state_map: HashMap<_, _> = self
            .modules
            .iter()
            .map(|(k, v)| (k.clone(), v.state.clone()))
            .collect();
        self.modules
            .values()
            .filter(|m| m.state == ModuleState::Ready && m.can_be_executed(&state_map))
            .collect()
    }
}
