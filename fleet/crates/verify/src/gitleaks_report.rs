use crate::{SecretFinding, VerifyError};
use serde::Deserialize;
use std::fs::File;
use std::io::Read;
use std::path::Path;

const MAX_REPORT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Deserialize)]
struct Finding {
    #[serde(rename = "RuleID")]
    rule_id: String,
    #[serde(rename = "File")]
    file: String,
    #[serde(rename = "Severity")]
    severity: Option<String>,
}

pub(super) fn parse(path: &Path) -> Result<Vec<SecretFinding>, VerifyError> {
    let size = path
        .metadata()
        .map_err(|error| VerifyError::ScannerParse(error.to_string()))?
        .len();
    if size > MAX_REPORT_BYTES {
        return Err(VerifyError::ScannerParse(
            "scanner report exceeds 16 MiB".into(),
        ));
    }
    let mut body = Vec::with_capacity(size as usize);
    File::open(path)
        .map_err(|error| VerifyError::ScannerParse(error.to_string()))?
        .take(MAX_REPORT_BYTES + 1)
        .read_to_end(&mut body)
        .map_err(|error| VerifyError::ScannerParse(error.to_string()))?;
    if body.len() as u64 > MAX_REPORT_BYTES {
        return Err(VerifyError::ScannerParse(
            "scanner report exceeds 16 MiB".into(),
        ));
    }
    let findings: Vec<Finding> = serde_json::from_slice(&body)
        .map_err(|error| VerifyError::ScannerParse(error.to_string()))?;
    Ok(findings
        .into_iter()
        .map(|finding| SecretFinding {
            rule_id: finding.rule_id,
            severity: finding.severity.unwrap_or_else(|| "HIGH".into()),
            file: finding.file,
            redacted: true,
        })
        .collect())
}

#[cfg(test)]
#[path = "gitleaks_report_tests.rs"]
mod tests;
