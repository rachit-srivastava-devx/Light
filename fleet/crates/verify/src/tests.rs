use crate::types::GateSpec;
use crate::{
    FakeCoverageProvider, FakeFindingsProvider, FakeGateRunner, ReviewedCandidate, Status, verify,
};

fn candidate(gates: Vec<GateSpec>) -> ReviewedCandidate {
    ReviewedCandidate {
        tree_digest: "t1".into(),
        acceptance_digest: "a1".into(),
        gates,
        coverage_floor: None,
        output_digest: None,
    }
}
fn spec(id: &str) -> GateSpec {
    GateSpec {
        id: id.into(),
        command: "true".into(),
        args: vec![],
    }
}
fn run(
    c: &ReviewedCandidate,
    pass: bool,
    cov: u64,
    fp: FakeFindingsProvider,
) -> crate::GateEvidence {
    let runner = if pass {
        FakeGateRunner::passing()
    } else {
        FakeGateRunner::failing()
    };
    verify(c, &runner, &fp, &FakeCoverageProvider::with(cov)).unwrap()
}

#[test]
fn reviewed_candidate_runs_deterministic_gates() {
    let ev = run(
        &candidate(vec![spec("g1")]),
        true,
        100,
        FakeFindingsProvider::empty(),
    );
    assert!(!ev.gate_results.is_empty());
    assert!(ev.checked > 0);
    assert_eq!(ev.checked, ev.total);
    assert_ne!(ev.status, Status::Refused);
}

#[test]
fn failing_gate_produces_gate_evidence_not_panic() {
    let ev = run(
        &candidate(vec![spec("g1")]),
        false,
        100,
        FakeFindingsProvider::empty(),
    );
    assert!(!ev.passed);
    assert!(!ev.failures.is_empty());
}

#[path = "tests_edge_cases.rs"]
mod edge_cases;
