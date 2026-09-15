use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

#[path = "gitleaks_binary.rs"]
mod binary;
#[path = "gitleaks_config.rs"]
mod config;
#[path = "gitleaks_git.rs"]
mod git;
#[path = "gitleaks_process.rs"]
mod process;
#[path = "gitleaks_report.rs"]
mod report;
#[path = "gitleaks_scan.rs"]
mod scan;
#[path = "gitleaks_scope.rs"]
mod scope;
#[path = "gitleaks_scope_files.rs"]
mod scope_files;
pub use scope::SecretScanScope;

pub struct GitleaksFindingsProvider {
    repo: PathBuf,
    binary: Option<PathBuf>,
    budget: Duration,
    git_required: bool,
    last_scope: Mutex<Option<SecretScanScope>>,
}

impl GitleaksFindingsProvider {
    pub fn new(repo: impl AsRef<Path>) -> Self {
        Self {
            repo: repo.as_ref().to_path_buf(),
            binary: None,
            budget: config::budget(),
            git_required: true,
            last_scope: Mutex::new(None),
        }
    }

    pub fn with_binary(repo: impl AsRef<Path>, binary: impl AsRef<Path>) -> Self {
        Self {
            repo: repo.as_ref().to_path_buf(),
            binary: Some(binary.as_ref().to_path_buf()),
            budget: config::budget(),
            git_required: true,
            last_scope: Mutex::new(None),
        }
    }

    pub fn with_budget(mut self, budget: Duration) -> Self {
        self.budget = budget;
        self
    }

    pub fn with_git_required(mut self, git_required: bool) -> Self {
        self.git_required = git_required;
        self
    }
}

#[cfg(test)]
#[path = "gitleaks_provider_tests.rs"]
mod provider_tests;
