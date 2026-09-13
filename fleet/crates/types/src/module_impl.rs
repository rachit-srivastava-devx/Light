use std::collections::HashMap;

use super::{Module, ModuleState};

impl Module {
    /// Create a new module with pending state
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        sow_text: impl Into<String>,
    ) -> Self {
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
        self.depends_on.iter().all(|dep_id| {
            matches!(
                state_map.get(dep_id),
                Some(ModuleState::Executed { .. } | ModuleState::Merged { .. })
            )
        })
    }
}
