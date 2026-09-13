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

// TODO: Integrate with fleet-types::Module once parallel module execution is fully wired
#[allow(dead_code)]
/// A unique identifier for a module within a pipeline run.
#[derive(Clone, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ModuleId(String);

#[allow(dead_code)]
impl ModuleId {
    pub fn parse(value: impl Into<String>) -> Result<Self, String> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err("module_id must not be empty".to_string());
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub struct StepLog {
    path: PathBuf,
}

impl StepLog {
    pub fn open(state_dir: &Path, task_id: &str) -> Self {
        Self {
            path: state_dir.join(format!("{task_id}.steps.json")),
        }
    }

    /// Open a step log for a specific module within a task.
    #[allow(dead_code)]
    pub fn open_module(state_dir: &Path, task_id: &str, module_id: &str) -> Self {
        Self {
            path: state_dir.join(format!("{task_id}.{module_id}.steps.json")),
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

    /// Check if a module stage is done (for parallel module execution).
    #[allow(dead_code)]
    pub fn is_module_done(&self, module_id: &str, stage: PipelineStage) -> bool {
        let module_log = Self::open_module(
            self.path.parent().unwrap(),
            self.path.file_stem().unwrap().to_string_lossy().as_ref(),
            module_id,
        );
        module_log.is_done(stage)
    }

    /// Mark a module stage as done (for parallel module execution).
    #[allow(dead_code)]
    pub fn mark_module_done(&self, module_id: &str, stage: PipelineStage) -> std::io::Result<()> {
        let module_log = Self::open_module(
            self.path.parent().unwrap(),
            self.path.file_stem().unwrap().to_string_lossy().as_ref(),
            module_id,
        );
        module_log.mark_done(stage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::stage::PipelineStage;

    #[test]
    fn a_marked_stage_survives_a_fresh_handle_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let log = StepLog::open(dir.path(), "t1");
        assert!(!log.is_done(PipelineStage::Scan));
        log.mark_done(PipelineStage::Scan).unwrap();

        let reopened = StepLog::open(dir.path(), "t1");
        assert!(reopened.is_done(PipelineStage::Scan));
        assert!(!reopened.is_done(PipelineStage::Plan));
    }

    #[test]
    fn module_step_log_isolated_per_module() {
        let dir = tempfile::tempdir().unwrap();
        let log = StepLog::open(dir.path(), "task-1");

        // Mark module-a as done
        log.mark_module_done("module-a", PipelineStage::Plan)
            .unwrap();

        // Module-a should be done
        assert!(log.is_module_done("module-a", PipelineStage::Plan));

        // Module-b should not be done yet
        assert!(!log.is_module_done("module-b", PipelineStage::Plan));

        // Mark module-b as done
        log.mark_module_done("module-b", PipelineStage::Plan)
            .unwrap();

        // Both should be done
        assert!(log.is_module_done("module-a", PipelineStage::Plan));
        assert!(log.is_module_done("module-b", PipelineStage::Plan));
    }

    #[test]
    fn module_id_validation() {
        assert!(ModuleId::parse("valid-module").is_ok());
        assert!(ModuleId::parse("").is_err());
        assert!(ModuleId::parse("   ").is_err());
    }
}
