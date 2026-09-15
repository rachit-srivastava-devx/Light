use super::*;

#[test]
fn checked_in_golden_cases_have_deterministic_outputs() {
    let cases: Vec<Case> = serde_json::from_str(include_str!("golden.json")).unwrap();
    assert!(!cases.is_empty());
    for case in cases {
        let candidate = ReviewedCandidate {
            tree_digest: case.tree_digest,
            acceptance_digest: case.acceptance_digest,
            gates: vec![CanonicalGateSpec {
                id: case.gate.id.clone(),
                command: "true".into(),
                args: vec![],
            }],
            coverage_floor: None,
            output_digest: None,
        };
        let gate = CanonicalGateResult {
            id: case.gate.id,
            exit_code: case.gate.exit_code,
            stdout_digest: case.gate.stdout_digest,
            stderr_digest: case.gate.stderr_digest,
            input_digest: case.gate.input_digest,
            checked: 1,
            total: 1,
            passed: case.gate.passed,
            failure_message: case.gate.failure_message,
        };
        let actual = assemble_gate_evidence(&candidate, vec![gate], case.findings);
        assert_eq!(
            actual.evidence_digest, actual.integrity_digest,
            "{}",
            case.name
        );
        assert_eq!(
            actual.evidence_digest, case.expected.evidence_digest,
            "{}",
            case.name
        );
        assert_eq!(actual.status, case.expected.status, "{}", case.name);
        assert_eq!(actual.passed, case.expected.passed, "{}", case.name);
        assert_eq!(
            (actual.checked, actual.total),
            (case.expected.checked, case.expected.total),
            "{}",
            case.name
        );
    }
}
