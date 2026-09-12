use crate::{Finding, ReviewError};

/// Filter findings to only those whose path is in the given scope list.
/// If scope is empty, all findings are returned unchanged.
pub fn apply_scope_filter(findings: Vec<Finding>, scope: &[String]) -> Vec<Finding> {
    if scope.is_empty() {
        return findings;
    }
    findings.into_iter().filter(|f| scope.contains(&f.path)).collect()
}

/// Invoke the `semgrep` binary and parse its JSON output into findings.
pub fn run_semgrep(path: &str) -> Result<Vec<Finding>, ReviewError> {
    let output = std::process::Command::new("semgrep")
        .args(["scan", "--json", "--config", "auto", path])
        .output()
        .map_err(|e| ReviewError::ToolUnavailable(format!("semgrep: {e}")))?;
    let raw: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| ReviewError::Malformed(format!("semgrep json: {e}")))?;
    parse_semgrep_findings(&raw)
}

/// Invoke the `ruff` binary and parse its JSON output into findings.
pub fn run_ruff(path: &str) -> Result<Vec<Finding>, ReviewError> {
    let output = std::process::Command::new("ruff")
        .args(["check", "--output-format", "json", path])
        .output()
        .map_err(|e| ReviewError::ToolUnavailable(format!("ruff: {e}")))?;
    let raw: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| ReviewError::Malformed(format!("ruff json: {e}")))?;
    parse_ruff_findings(&raw)
}

pub fn parse_semgrep_findings(raw: &serde_json::Value) -> Result<Vec<Finding>, ReviewError> {
    let results = raw
        .get("results")
        .and_then(|r| r.as_array())
        .ok_or_else(|| ReviewError::Malformed("semgrep: no 'results' array".to_string()))?;
    let mut out = Vec::new();
    for item in results {
        let severity = item
            .get("extra").and_then(|e| e.get("severity"))
            .and_then(|s| s.as_str()).unwrap_or("INFO").to_string();
        let path = item
            .get("path").and_then(|p| p.as_str()).unwrap_or("").to_string();
        let rationale = item
            .get("extra").and_then(|e| e.get("message"))
            .and_then(|m| m.as_str()).unwrap_or("").to_string();
        out.push(Finding { severity, path, rationale });
    }
    Ok(out)
}

pub fn parse_ruff_findings(raw: &serde_json::Value) -> Result<Vec<Finding>, ReviewError> {
    let results = raw
        .as_array()
        .ok_or_else(|| ReviewError::Malformed("ruff: expected JSON array".to_string()))?;
    let mut out = Vec::new();
    for item in results {
        let path = item
            .get("filename").and_then(|p| p.as_str()).unwrap_or("").to_string();
        let rationale = item
            .get("message").and_then(|m| m.as_str()).unwrap_or("").to_string();
        out.push(Finding { severity: "WARNING".to_string(), path, rationale });
    }
    Ok(out)
}
