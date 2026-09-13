//! Module-level step tracking (see `step_log.rs`'s own doc comment): tracking individual modules
//! within a parallel execution, for crash-resume of module-level SOW/planning/execution. Not yet
//! wired -- kept alongside `ModuleId` (`step_log_module_id.rs`) until parallel module execution
//! lands for real.

use super::super::stage::PipelineStage;
use super::StepLog;
use std::path::Path;

impl StepLog {
    /// Open a step log for a specific module within a task.
    #[allow(dead_code)]
    pub fn open_module(state_dir: &Path, task_id: &str, module_id: &str) -> Self {
        Self {
            path: state_dir.join(format!("{task_id}.{module_id}.steps.json")),
        }
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
