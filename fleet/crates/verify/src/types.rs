use serde::{Deserialize, Serialize};

#[path = "error.rs"]
mod error;
pub use error::VerifyError;

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
    /// Measured work units. A passing result is invalid unless `total > 0` and `checked <= total`.
    #[serde(default)]
    pub checked: u64,
    #[serde(default)]
    pub total: u64,
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
    Mismatch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateEvidence {
    pub gate_results: Vec<GateResult>,
    pub checked: u64,
    pub total: u64,
    /// Compatibility alias for `integrity_digest`.  New consumers must use
    /// `integrity_digest`; retaining this field avoids a breaking deserialization change while
    /// removing the old fabricated hash implementation.
    pub evidence_digest: String,
    /// Canonical cryptographic digest for the complete candidate and evidence.
    pub integrity_digest: String,
    pub status: Status,
    pub passed: bool,
    pub failures: Vec<String>,
    pub findings: Vec<SecretFinding>,
}
