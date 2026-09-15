use crate::types::{GateEvidence, GateResult, ReviewedCandidate, SecretFinding, Status};

use super::{denominator::valid_input_denominator, integrity};

pub fn assemble_gate_evidence(
    candidate: &ReviewedCandidate,
    results: Vec<GateResult>,
    findings: Vec<SecretFinding>,
) -> GateEvidence {
    let total = results.len() as u64;
    let checked = total;
    let mut failures: Vec<String> = Vec::new();

    if total == 0 {
        failures.push("no gate inputs were checked".into());
    }

    for r in &results {
        if r.total == 0 && r.checked == 0 {
            failures.push(format!("gate '{}' published an empty denominator", r.id));
        } else if r.checked > r.total {
            failures.push(format!(
                "gate '{}' published invalid denominator {}/{}",
                r.id, r.checked, r.total
            ));
        }
        if r.passed && !valid_input_denominator(&r.input_digest, r.checked, r.total) {
            failures.push(format!(
                "gate '{}' did not publish a matching typed denominator",
                r.id
            ));
        }
        if !r.passed {
            let msg = r
                .failure_message
                .clone()
                .unwrap_or_else(|| format!("gate '{}' failed (exit {})", r.id, r.exit_code));
            failures.push(msg);
        }
    }

    if !findings.is_empty() {
        failures.push(format!("{} secret finding(s) blocked gate", findings.len()));
    }

    let integrity_digest = integrity::compute(candidate, &results, &findings);
    // Keep the compatibility alias cryptographically identical to the canonical digest.
    let evidence_digest = integrity_digest.clone();

    if let Some(expected) = &candidate.output_digest {
        if *expected != integrity_digest {
            failures.push(format!(
                "digest mismatch: expected={expected} computed={integrity_digest}"
            ));
        }
    }

    let digest_mismatch = failures.iter().any(|f| f.starts_with("digest mismatch:"));
    let passed = failures.is_empty();
    let status = if digest_mismatch {
        Status::Mismatch
    } else if passed {
        Status::Passed
    } else {
        Status::Failed
    };

    GateEvidence {
        gate_results: results,
        checked,
        total,
        evidence_digest,
        integrity_digest,
        status,
        passed,
        failures,
        findings,
    }
}
