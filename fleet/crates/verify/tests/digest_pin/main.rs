use verify::{assemble_gate_evidence, CanonicalGateResult, ReviewedCandidate, SecretFinding};

fn candidate() -> ReviewedCandidate {
    ReviewedCandidate {
        tree_digest: "t1".into(),
        acceptance_digest: "a1".into(),
        gates: vec![],
        coverage_floor: None,
        output_digest: None,
    }
}

fn gate_result(id: &str, exit_code: i32, stdout_digest: &str) -> CanonicalGateResult {
    CanonicalGateResult {
        id: id.into(),
        exit_code,
        stdout_digest: stdout_digest.into(),
        stderr_digest: "".into(),
        input_digest: "denominator:1/1".into(),
        checked: 1,
        total: 1,
        passed: exit_code == 0,
        failure_message: None,
    }
}

#[test]
fn compatibility_evidence_digest_is_the_canonical_integrity_digest() {
    let ev = assemble_gate_evidence(&candidate(), vec![gate_result("g1", 0, "abc")], vec![]);
    assert_eq!(ev.evidence_digest, ev.integrity_digest);
    assert!(ev.integrity_digest.starts_with("blake3:"));
}

#[test]
fn findings_change_both_digest_aliases() {
    let without = assemble_gate_evidence(&candidate(), vec![gate_result("g1", 0, "abc")], vec![]);
    let with_finding = assemble_gate_evidence(
        &candidate(),
        vec![gate_result("g1", 0, "abc")],
        vec![SecretFinding {
            rule_id: "AWS_KEY".into(),
            severity: "critical".into(),
            file: "src/lib.rs".into(),
            redacted: true,
        }],
    );
    assert_ne!(without.evidence_digest, with_finding.evidence_digest);
    assert_eq!(without.evidence_digest, without.integrity_digest);
    assert_eq!(with_finding.evidence_digest, with_finding.integrity_digest);
}
