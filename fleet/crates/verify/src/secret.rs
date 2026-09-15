use crate::{SecretFinding, VerifyError};
#[path = "gitleaks.rs"]
mod gitleaks;
pub use gitleaks::GitleaksFindingsProvider;
pub use gitleaks::SecretScanScope;

pub trait FindingsProvider: Send + Sync {
    fn scan(&self, tree_digest: &str) -> Result<Vec<SecretFinding>, VerifyError>;
}

pub struct FakeFindingsProvider {
    findings: Vec<SecretFinding>,
}

impl FakeFindingsProvider {
    pub fn empty() -> Self {
        Self { findings: vec![] }
    }

    pub fn with_critical(file: &str) -> Self {
        Self {
            findings: vec![SecretFinding {
                rule_id: "generic-api-key".into(),
                severity: "CRITICAL".into(),
                file: file.into(),
                redacted: false,
            }],
        }
    }
}

impl FindingsProvider for FakeFindingsProvider {
    fn scan(&self, _tree_digest: &str) -> Result<Vec<SecretFinding>, VerifyError> {
        Ok(self.findings.clone())
    }
}

pub fn normalize_findings(raw: Vec<SecretFinding>) -> Vec<SecretFinding> {
    raw.into_iter()
        .map(|mut f| {
            f.redacted = true;
            f
        })
        .collect()
}

pub fn findings_summary(findings: &[SecretFinding]) -> String {
    findings
        .iter()
        .map(|finding| {
            format!(
                "rule={} severity={} file={} redacted={}",
                finding.rule_id, finding.severity, finding.file, finding.redacted
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

pub fn scan_secrets(repo: impl AsRef<std::path::Path>) -> Result<Vec<SecretFinding>, VerifyError> {
    scan_secrets_with_mode(repo, true)
}

pub fn scan_secrets_with_mode(
    repo: impl AsRef<std::path::Path>,
    git_required: bool,
) -> Result<Vec<SecretFinding>, VerifyError> {
    let provider = GitleaksFindingsProvider::new(repo).with_git_required(git_required);
    Ok(normalize_findings(provider.scan_with_mode(git_required)?))
}
