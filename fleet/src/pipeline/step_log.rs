//! **Restate deferred** (see top-level flag in `pipeline/mod.rs`): a resumable step log stands
//! in for `restate_sdk::Context::run`'s journal. It records, on disk, which `PipelineStage`s a
//! given `TaskId` has already completed, so a re-run after a crash skips finished stages instead
//! of re-executing them -- the one durability property BLUEPRINT §6's "partial-failure" row
//! requires, without pulling in the full SDK in this pass.
//!
//! **Module-level step logging**: Extended to support tracking individual modules within a
//! parallel execution, allowing crash-resume for module-level SOW, planning, and execution.

use super::stage::PipelineStage;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[path = "step_log_module_id.rs"]
mod step_log_module_id;
#[path = "step_log_module.rs"]
mod step_log_module;
#[allow(unused_imports)] // not yet consumed outside this module -- see its own TODO
pub use step_log_module_id::ModuleId;

pub struct StepLog {
    path: PathBuf,
}

impl StepLog {
    pub fn open(state_dir: &Path, task_id: &str) -> Self {
        Self {
            path: state_dir.join(format!("{task_id}.steps.json")),
        }
    }

    #[allow(dead_code)]
    fn load(&self) -> BTreeSet<PipelineStage> {
        let Ok(text) = fs::read_to_string(&self.path) else {
            return BTreeSet::new();
        };
        serde_json::from_str(&text).unwrap_or_default()
    }

    pub fn is_done(&self, stage: PipelineStage) -> bool {
        self.load().contains(&stage)
    }

    pub fn mark_done(&self, stage: PipelineStage) -> std::io::Result<()> {
        let mut done = self.load();
        done.insert(stage);
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string(&done).expect("BTreeSet<PipelineStage> always encodes");
        fs::write(&self.path, text)
    }
}

#[cfg(test)]
#[path = "step_log_tests.rs"]
mod tests;
