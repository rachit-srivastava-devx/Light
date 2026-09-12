use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateSpec {
    pub id: String,
    pub command: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewedCandidate {
    pub tree_digest: String,
    pub acceptance_digest: String,
    pub gates: Vec<GateSpec>,
    pub coverage_floor: Option<u64>,
    pub output_digest: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResult {
    pub id: String,
    pub exit_code: i32,
    pub stdout_digest: String,
    pub stderr_digest: String,
    pub input_digest: String,
    pub passed: bool,
    pub failure_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretFinding {
    pub rule_id: String,
    pub severity: String,
    pub file: String,
    pub redacted: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Status {
    Passed,
    Failed,
    Refused,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateEvidence {
    pub gate_results: Vec<GateResult>,
    pub checked: u64,
    pub total: u64,
    pub evidence_digest: String,
    pub status: Status,
    pub passed: bool,
    pub failures: Vec<String>,
    pub findings: Vec<SecretFinding>,
}

#[derive(Debug, Error)]
pub enum VerifyError {
    #[error("no gates specified")]
    NoGates,
    #[error("gate invalid: {0}")]
    InvalidGate(String),
    #[error("coverage error: {0}")]
    CoverageParseError(String),
    #[error("secret found")]
    SecretFound,
}
