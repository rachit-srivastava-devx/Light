use crate::types::{GateEvidence, GateResult, ReviewedCandidate, SecretFinding, Status};

fn hash_str(s: &str) -> u64 {
    let mut h: u64 = 14_695_981_039_346_656_037;
    for b in s.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(1_099_511_628_211);
    }
    h
}

fn compute_digest(results: &[GateResult], findings: &[SecretFinding]) -> String {
    let mut parts: Vec<String> = results
        .iter()
        .map(|r| format!("{}:{}:{}", r.id, r.exit_code, r.stdout_digest))
        .collect();
    for f in findings {
        parts.push(format!("secret:{}:{}", f.rule_id, f.severity));
    }
    format!("sha256:{:x}", hash_str(&parts.join("|")))
}

pub fn assemble_gate_evidence(
    candidate: &ReviewedCandidate,
    results: Vec<GateResult>,
    findings: Vec<SecretFinding>,
) -> GateEvidence {
    let total = results.len() as u64;
    let checked = total;
    let mut failures: Vec<String> = Vec::new();

    for r in &results {
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

    let evidence_digest = compute_digest(&results, &findings);

    if let Some(expected) = &candidate.output_digest {
        if *expected != evidence_digest {
            failures.push(format!(
                "digest mismatch: expected={expected} computed={evidence_digest}"
            ));
        }
    }

    let passed = failures.is_empty();
    let status = if passed {
        Status::Passed
    } else {
        Status::Failed
    };

    GateEvidence {
        gate_results: results,
        checked,
        total,
        evidence_digest,
        status,
        passed,
        failures,
        findings,
    }
}
