use crate::{
    CanonicalGateResult, CanonicalGateSpec, FindingsProvider, assemble_gate_evidence,
    normalize_findings,
};
use crate::{
    FakeCoverageProvider, FakeFindingsProvider, FakeGateRunner, ReviewedCandidate, Status,
};

fn candidate() -> ReviewedCandidate {
    ReviewedCandidate {
        tree_digest: "tree-matrix".into(),
        acceptance_digest: "acceptance-matrix".into(),
        gates: vec![CanonicalGateSpec {
            id: "matrix".into(),
            command: "true".into(),
            args: vec![],
        }],
        coverage_floor: None,
        output_digest: None,
    }
}

fn result(passed: bool, input_digest: &str) -> CanonicalGateResult {
    let (checked, total) = input_digest
        .strip_prefix("denominator:")
        .and_then(|value| value.split_once('/'))
        .map(|(checked, total)| (checked.parse().unwrap(), total.parse().unwrap()))
        .unwrap_or((0, 0));
    CanonicalGateResult {
        id: "matrix".into(),
        exit_code: if passed { 0 } else { 6 },
        stdout_digest: "blake3:stdout".into(),
        stderr_digest: "blake3:stderr".into(),
        input_digest: input_digest.into(),
        checked,
        total,
        passed,
        failure_message: (!passed).then(|| "deliberate failure".into()),
    }
}

#[test]
fn digest_binds_gate_output_and_denominator() {
    let c = candidate();
    let first = assemble_gate_evidence(&c, vec![result(true, "denominator:2/2")], vec![]);
    let changed = assemble_gate_evidence(&c, vec![result(true, "denominator:1/2")], vec![]);
    assert_ne!(first.integrity_digest, changed.integrity_digest);
    assert_eq!((first.checked, first.total), (1, 1));
}

#[test]
fn zero_inputs_and_failed_denominator_never_pass() {
    let c = candidate();
    let empty = assemble_gate_evidence(&c, vec![], vec![]);
    let failed = assemble_gate_evidence(&c, vec![result(false, "denominator:0/2")], vec![]);
    assert!(!empty.passed);
    assert_eq!((empty.checked, empty.total), (0, 0));
    assert!(!failed.passed);
    assert_eq!(failed.status, Status::Failed);
}

#[test]
fn canonical_receipt_rejects_a_per_gate_empty_denominator() {
    let c = candidate();
    let evidence = assemble_gate_evidence(&c, vec![result(true, "denominator:0/0")], vec![]);
    assert!(!evidence.passed);
    assert_eq!(evidence.status, Status::Failed);
    assert!(
        evidence
            .failures
            .iter()
            .any(|failure| failure.contains("empty denominator"))
    );
}

#[path = "matrix_evidence_tests.rs"]
mod evidence;
