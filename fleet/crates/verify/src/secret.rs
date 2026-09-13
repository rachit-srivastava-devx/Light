use crate::{SecretFinding, VerifyError};

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
