use crate::{
    verify, FakeCoverageProvider, FakeFindingsProvider, FakeGateRunner,
    ReviewedCandidate, Status,
};
use crate::types::GateSpec;

fn candidate(gates: Vec<GateSpec>) -> ReviewedCandidate {
    ReviewedCandidate {
        tree_digest: "t1".into(), acceptance_digest: "a1".into(),
        gates, coverage_floor: None, output_digest: None,
    }
}
fn spec(id: &str) -> GateSpec { GateSpec { id: id.into(), command: "true".into(), args: vec![] } }
fn run(c: &ReviewedCandidate, pass: bool, cov: f64, fp: FakeFindingsProvider)
    -> crate::GateEvidence
{
    let runner = if pass { FakeGateRunner::passing() } else { FakeGateRunner::failing() };
    verify(c, &runner, &fp, &FakeCoverageProvider::with(cov)).unwrap()
}

#[test]
fn reviewed_candidate_runs_deterministic_gates() {
    let ev = run(&candidate(vec![spec("g1")]), true, 100.0, FakeFindingsProvider::empty());
    assert!(!ev.gate_results.is_empty());
    assert!(ev.checked > 0);
    assert_eq!(ev.checked, ev.total);
    assert_ne!(ev.status, Status::Refused);
}

#[test]
fn failing_gate_produces_gate_evidence_not_panic() {
    let ev = run(&candidate(vec![spec("g1")]), false, 100.0, FakeFindingsProvider::empty());
    assert!(!ev.passed);
    assert!(!ev.failures.is_empty());
}

#[test]
fn coverage_below_floor_marks_gate_failed() {
    let mut c = candidate(vec![spec("g1")]);
    c.coverage_floor = Some(80);
    let ev = run(&c, true, 70.0, FakeFindingsProvider::empty());
    let cov = ev.gate_results.iter().find(|r| r.id == "coverage").unwrap();
    assert!(!cov.passed);
    let msg = cov.failure_message.as_deref().unwrap_or("");
    assert!(msg.contains("70"), "msg={msg}");
    assert!(msg.contains("80"), "msg={msg}");
}

#[test]
fn secret_finding_blocks_gate() {
    let ev = run(&candidate(vec![spec("g1")]), true, 100.0,
        FakeFindingsProvider::with_critical("secrets.txt"));
    assert!(!ev.passed);
    assert!(!ev.findings.is_empty());
}

#[test]
fn evidence_digest_mismatch_refused() {
    let mut c = candidate(vec![spec("g1")]);
    c.output_digest = Some("wrong-digest".into());
    let ev = run(&c, true, 100.0, FakeFindingsProvider::empty());
    assert!(!ev.passed);
    let found = ev.failures.iter().any(|f| f.contains("mismatch") || f.contains("digest"));
    assert!(found, "failures: {:?}", ev.failures);
}
