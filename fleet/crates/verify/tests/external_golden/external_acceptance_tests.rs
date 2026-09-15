use super::*;

#[test]
fn external_dataset_denominator_is_accepted() {
    let manifest: Manifest = serde_json::from_str(include_str!("golden_manifest.json")).unwrap();
    let total = manifest.expected.records as u64;
    let candidate = ReviewedCandidate {
        tree_digest: "external-golden-tree".into(),
        acceptance_digest: "external-golden-acceptance".into(),
        gates: vec![CanonicalGateSpec {
            id: "golden-records".into(),
            command: "fixture".into(),
            args: vec![],
        }],
        coverage_floor: None,
        output_digest: None,
    };
    let gate = CanonicalGateResult {
        id: "golden-records".into(),
        exit_code: 0,
        stdout_digest: manifest.expected.blake3,
        stderr_digest: "blake3:empty".into(),
        input_digest: format!("denominator:{total}/{total}"),
        checked: total,
        total,
        passed: true,
        failure_message: None,
    };
    let evidence = assemble_gate_evidence(&candidate, vec![gate], vec![]);
    assert_eq!(evidence.status, Status::Passed);
    assert_eq!((evidence.checked, evidence.total), (1, 1));
}
