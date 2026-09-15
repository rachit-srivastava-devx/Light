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

#[path = "step_log_module.rs"]
mod step_log_module;
#[path = "step_log_module_id.rs"]
mod step_log_module_id;
#[path = "step_log_repo_key.rs"]
mod step_log_repo_key;
#[allow(unused_imports)] // not yet consumed outside this module -- see its own TODO
pub use step_log_module_id::ModuleId;
use step_log_repo_key::repo_key;

pub struct StepLog {
    path: PathBuf,
}

impl StepLog {
    /// Task-only, no repo scoping -- kept for `step_log_tests.rs` to exercise the underlying
    /// resume/reopen mechanic in isolation. Production must go through `open_for_repo`: an
    /// unscoped log is exactly the defect this file exists to prevent (see its doc comment).
    #[cfg(test)]
    fn open(state_dir: &Path, task_id: &str) -> Self {
        Self {
            path: state_dir.join(format!("{task_id}.steps.json")),
        }
    }

    /// The step log for one (task, repo) pair -- what `run_pipeline` must use so N `--repo`s
    /// under one task_id in one `fleet run` invocation get N independent step logs instead of
    /// silently sharing (and short-circuiting on) one.
    pub fn open_for_repo(state_dir: &Path, task_id: &str, repo: &Path) -> Self {
        Self {
            path: state_dir.join(format!("{task_id}.{}.steps.json", repo_key(repo))),
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
