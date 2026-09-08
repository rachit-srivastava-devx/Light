//! `LoopStore` fake shared by the autonomous-run integration tests -- an in-memory stand-in for
//! `FileLoopStore` used where the test doesn't need real cross-process durability.

use std::collections::BTreeMap;
use std::sync::Mutex;

use fleet_govern::{LoopIoError, LoopProgress, LoopStore};

pub struct InMemoryLoopStore {
    rows: Mutex<BTreeMap<String, LoopProgress>>,
}

impl InMemoryLoopStore {
    pub fn new() -> Self {
        Self { rows: Mutex::new(BTreeMap::new()) }
    }
}

impl LoopStore for InMemoryLoopStore {
    fn load(&self, plan_id: &str) -> Result<Option<LoopProgress>, LoopIoError> {
        Ok(self.rows.lock().unwrap().get(plan_id).cloned())
    }

    fn save(&self, plan_id: &str, progress: &LoopProgress) -> Result<(), LoopIoError> {
        self.rows.lock().unwrap().insert(plan_id.to_string(), progress.clone());
        Ok(())
    }
}
