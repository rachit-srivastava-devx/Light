use crate::types::{GateResult, ReviewedCandidate, SecretFinding};

fn field(hasher: &mut blake3::Hasher, value: &str) {
    hasher.update(&(value.len() as u64).to_le_bytes());
    hasher.update(value.as_bytes());
}

pub(super) fn compute(
    candidate: &ReviewedCandidate,
    results: &[GateResult],
    findings: &[SecretFinding],
) -> String {
    let mut hasher = blake3::Hasher::new();
    field(&mut hasher, &candidate.tree_digest);
    field(&mut hasher, &candidate.acceptance_digest);
    for gate in &candidate.gates {
        field(&mut hasher, &gate.id);
        field(&mut hasher, &gate.command);
        for arg in &gate.args {
            field(&mut hasher, arg);
        }
    }
    field(
        &mut hasher,
        &candidate
            .coverage_floor
            .map_or_else(String::new, |floor| floor.to_string()),
    );
    for result in results {
        field(&mut hasher, &result.id);
        field(&mut hasher, &result.exit_code.to_string());
        field(&mut hasher, &result.stdout_digest);
        field(&mut hasher, &result.stderr_digest);
        field(&mut hasher, &result.input_digest);
        field(&mut hasher, &result.passed.to_string());
        field(&mut hasher, result.failure_message.as_deref().unwrap_or(""));
    }
    for finding in findings {
        field(&mut hasher, &finding.rule_id);
        field(&mut hasher, &finding.severity);
        field(&mut hasher, &finding.file);
        field(&mut hasher, &finding.redacted.to_string());
    }
    format!("blake3:{}", hasher.finalize().to_hex())
}
