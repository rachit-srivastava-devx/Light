use crate::VerifyError;
use std::path::Path;
use std::time::Duration;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SecretScanScope {
    checked: u64,
    total: u64,
    git_backed: bool,
    requested_git: bool,
    no_head_fallback: bool,
}

impl SecretScanScope {
    pub fn checked(self) -> u64 {
        self.checked
    }

    pub fn total(self) -> u64 {
        self.total
    }

    pub fn git_backed(self) -> bool {
        self.git_backed
    }

    pub fn requested_git(self) -> bool {
        self.requested_git
    }

    pub fn no_head_fallback(self) -> bool {
        self.no_head_fallback
    }
}

pub(super) fn measure(
    repo: &Path,
    git_required: bool,
    requested_git: bool,
    no_head_fallback: bool,
    budget: Duration,
) -> Result<SecretScanScope, VerifyError> {
    let total = if git_required {
        super::git::commit_count(repo, budget)?
    } else {
        super::scope_files::file_count(repo)?
    };
    if total == 0 {
        return Err(VerifyError::ScannerScopeUnavailable(
            "secret scanner scope is empty".into(),
        ));
    }
    Ok(SecretScanScope {
        checked: total,
        total,
        git_backed: git_required,
        requested_git,
        no_head_fallback,
    })
}
