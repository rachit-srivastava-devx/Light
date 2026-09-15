use serde::Deserialize;
use verify::{
    CanonicalGateResult, CanonicalGateSpec, ReviewedCandidate, Status, assemble_gate_evidence,
};

#[derive(Deserialize)]
struct Case {
    name: String,
    tree_digest: String,
    acceptance_digest: String,
    gate: Gate,
    findings: Vec<verify::SecretFinding>,
    expected: Expected,
}

#[derive(Deserialize)]
struct Gate {
    id: String,
    exit_code: i32,
    stdout_digest: String,
    stderr_digest: String,
    input_digest: String,
    passed: bool,
    failure_message: Option<String>,
}

#[derive(Deserialize)]
struct Expected {
    evidence_digest: String,
    status: Status,
    passed: bool,
    checked: u64,
    total: u64,
}

#[path = "golden_cases.rs"]
mod cases;
