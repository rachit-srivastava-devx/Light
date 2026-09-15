use super::*;

#[test]
fn canonical_receipt_rejects_an_opaque_passing_result() {
    let c = candidate();
    let evidence = assemble_gate_evidence(&c, vec![result(true, "sha256:opaque")], vec![]);
    assert!(!evidence.passed);
    assert!(
        evidence
            .failures
            .iter()
            .any(|failure| failure.contains("matching typed denominator"))
    );
}

#[test]
fn findings_are_redacted_and_change_integrity() {
    let c = candidate();
    let clean = assemble_gate_evidence(&c, vec![result(true, "denominator:1/1")], vec![]);
    let findings = normalize_findings(
        FakeFindingsProvider::with_critical("secret.env")
            .scan("t")
            .unwrap(),
    );
    assert!(findings.iter().all(|finding| finding.redacted));
    let blocked = assemble_gate_evidence(&c, vec![result(true, "denominator:1/1")], findings);
    assert!(!blocked.passed);
    assert_ne!(clean.integrity_digest, blocked.integrity_digest);
}

#[test]
fn canonical_runner_and_coverage_contracts_are_exercised() {
    let c = candidate();
    let evidence = crate::verify(
        &c,
        &FakeGateRunner::passing(),
        &FakeFindingsProvider::empty(),
        &FakeCoverageProvider::with(100),
    )
    .unwrap();
    assert!(evidence.passed);
    assert_eq!(evidence.checked, evidence.total);
}

#[test]
fn mutation_sensitivity_catches_a_vacuous_pass() {
    fn weakened(_candidate: &ReviewedCandidate) -> bool {
        true
    }
    let c = candidate();
    let real = assemble_gate_evidence(&c, vec![], vec![]).passed;
    assert!(!real);
    assert!(weakened(&c));
    assert_ne!(
        real,
        weakened(&c),
        "mutation must be killed by this assertion"
    );
}

#[test]
fn legacy_result_without_measured_fields_fails_closed() {
    let legacy = serde_json::json!({
        "id": "matrix",
        "exit_code": 0,
        "stdout_digest": "stdout",
        "stderr_digest": "stderr",
        "input_digest": "denominator:1/1",
        "passed": true,
        "failure_message": null
    });
    let result: CanonicalGateResult = serde_json::from_value(legacy).unwrap();
    let evidence = assemble_gate_evidence(&candidate(), vec![result], vec![]);
    assert!(!evidence.passed);
}
