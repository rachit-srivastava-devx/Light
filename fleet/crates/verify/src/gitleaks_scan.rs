use super::{binary, git, process, report, scope, GitleaksFindingsProvider, SecretScanScope};
use crate::{SecretFinding, VerifyError};

impl crate::FindingsProvider for GitleaksFindingsProvider {
    fn scan(&self, _tree_digest: &str) -> Result<Vec<SecretFinding>, VerifyError> {
        self.scan_with_mode(self.git_required)
    }
}

impl GitleaksFindingsProvider {
    pub fn scan_with_mode(&self, git_required: bool) -> Result<Vec<SecretFinding>, VerifyError> {
        let no_head_fallback = git_required && !git::has_head(&self.repo, self.budget)?;
        let git_backed = git_required && !no_head_fallback;
        let measured_scope = scope::measure(
            &self.repo,
            git_backed,
            git_required,
            no_head_fallback,
            self.budget,
        )?;
        let report = tempfile::NamedTempFile::new()
            .map_err(|error| VerifyError::ScannerUnavailable(error.to_string()))?;
        let binary = binary::resolve(self.binary.as_deref(), self.budget)?;
        let output = process::run(&binary, &self.repo, report.path(), self.budget, git_backed)?;
        if !output.status.success() {
            return Err(VerifyError::ScannerFailed {
                code: output.status.code(),
                stderr: process::diagnostic(&output.stderr),
            });
        }
        let findings = report::parse(report.path())?;
        *self
            .last_scope
            .lock()
            .map_err(|_| VerifyError::ScannerScopeUnavailable("scope lock poisoned".into()))? =
            Some(measured_scope);
        Ok(findings)
    }

    pub fn last_scope(&self) -> Result<SecretScanScope, VerifyError> {
        self.last_scope
            .lock()
            .map_err(|_| VerifyError::ScannerScopeUnavailable("scope lock poisoned".into()))?
            .ok_or_else(|| {
                VerifyError::ScannerScopeUnavailable("secret scan did not publish scope".into())
            })
    }
}
