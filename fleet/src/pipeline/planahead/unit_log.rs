//! Resumable on-disk progress log for the plan-ahead orchestrator -- the same shape as
//! `pipeline::step_log::StepLog` (BLUEPRINT §6's crash-resume property) but keyed by an
//! arbitrary unit id + `UnitPhase` instead of the fixed 8-stage `PipelineStage` enum, since a
//! "module" here is caller-defined, not one of the pipeline's own stages. Guarded by a
//! `std::sync::Mutex` because, unlike `StepLog` (called from one sequential stage loop), this is
//! read/written from concurrently spawned planner and builder tokio tasks.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub enum UnitPhase {
    Planned,
    Built,
}

pub struct UnitLog {
    path: PathBuf,
    lock: Mutex<()>,
}

impl UnitLog {
    pub fn open(state_dir: &Path, run_id: &str) -> Self {
        Self { path: state_dir.join(format!("{run_id}.planahead.json")), lock: Mutex::new(()) }
    }

    fn load(&self) -> BTreeSet<(String, UnitPhase)> {
        let Ok(text) = std::fs::read_to_string(&self.path) else { return BTreeSet::new() };
        serde_json::from_str(&text).unwrap_or_default()
    }

    pub fn is_done(&self, unit: &str, phase: UnitPhase) -> bool {
        let _guard = self.lock.lock().unwrap_or_else(|p| p.into_inner());
        self.load().contains(&(unit.to_string(), phase))
    }

    pub fn mark_done(&self, unit: &str, phase: UnitPhase) -> std::io::Result<()> {
        let _guard = self.lock.lock().unwrap_or_else(|p| p.into_inner());
        let mut done = self.load();
        done.insert((unit.to_string(), phase));
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string(&done)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(&self.path, text)
    }
}
