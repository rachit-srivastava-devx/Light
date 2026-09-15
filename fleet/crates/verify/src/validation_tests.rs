use crate::{CanonicalGateSpec, ReviewedCandidate, VerifyError};
use crate::{FakeCoverageProvider, FakeFindingsProvider, FakeGateRunner, verify};

fn candidate(gates: Vec<CanonicalGateSpec>) -> ReviewedCandidate {
    ReviewedCandidate {
        tree_digest: "tree".into(),
        acceptance_digest: "acceptance".into(),
        gates,
        coverage_floor: None,
        output_digest: None,
    }
}

fn gate(id: &str) -> CanonicalGateSpec {
    CanonicalGateSpec {
        id: id.into(),
        command: "true".into(),
        args: vec![],
    }
}

#[test]
fn rejects_missing_authority_digests_before_running() {
    let mut c = candidate(vec![gate("g1")]);
    c.tree_digest.clear();
    let err = verify(
        &c,
        &FakeGateRunner::passing(),
        &FakeFindingsProvider::empty(),
        &FakeCoverageProvider::with(100),
    )
    .unwrap_err();
    assert!(matches!(err, VerifyError::InvalidCandidate(_)));
}

#[test]
fn rejects_duplicate_gate_names_and_impossible_floor() {
    let mut c = candidate(vec![gate("g1"), gate("g1")]);
    let err = verify(
        &c,
        &FakeGateRunner::passing(),
        &FakeFindingsProvider::empty(),
        &FakeCoverageProvider::with(100),
    )
    .unwrap_err();
    assert!(matches!(err, VerifyError::InvalidGate(_)));
    c.gates = vec![gate("g1")];
    c.coverage_floor = Some(101);
    let err = verify(
        &c,
        &FakeGateRunner::passing(),
        &FakeFindingsProvider::empty(),
        &FakeCoverageProvider::with(100),
    )
    .unwrap_err();
    assert!(matches!(err, VerifyError::InvalidCandidate(_)));
}

#[test]
fn rejects_blank_gate_id_and_command_independently() {
    for (id, command) in [("", "true"), ("g1", "")] {
        let err = verify(
            &candidate(vec![CanonicalGateSpec {
                id: id.into(),
                command: command.into(),
                args: vec![],
            }]),
            &FakeGateRunner::passing(),
            &FakeFindingsProvider::empty(),
            &FakeCoverageProvider::with(100),
        )
        .unwrap_err();
        assert!(matches!(err, VerifyError::InvalidGate(_)));
    }
}

#[path = "validation_boundary_tests.rs"]
mod boundary;
