use crate::ReviewedCandidate;
use crate::{
    verify, CanonicalGateSpec, FakeCoverageProvider, FakeFindingsProvider, FakeGateRunner,
};

fn candidate(tree: &str, acceptance: &str) -> ReviewedCandidate {
    ReviewedCandidate {
        tree_digest: tree.into(),
        acceptance_digest: acceptance.into(),
        gates: vec![CanonicalGateSpec {
            id: "g1".into(),
            command: "true".into(),
            args: vec![],
        }],
        coverage_floor: None,
        output_digest: None,
    }
}

#[test]
fn integrity_digest_is_cryptographic_and_binds_authority_inputs() {
    let providers = || {
        (
            FakeGateRunner::passing(),
            FakeFindingsProvider::empty(),
            FakeCoverageProvider::with(100),
        )
    };
    let (runner, findings, coverage) = providers();
    let first = verify(
        &candidate("tree-a", "accept-a"),
        &runner,
        &findings,
        &coverage,
    )
    .unwrap();
    let (runner, findings, coverage) = providers();
    let second = verify(
        &candidate("tree-b", "accept-a"),
        &runner,
        &findings,
        &coverage,
    )
    .unwrap();
    let (runner, findings, coverage) = providers();
    let third = verify(
        &candidate("tree-a", "accept-b"),
        &runner,
        &findings,
        &coverage,
    )
    .unwrap();
    assert!(first.integrity_digest.starts_with("blake3:"));
    assert_eq!(first.integrity_digest.len(), 71);
    assert_ne!(first.integrity_digest, second.integrity_digest);
    assert_ne!(first.integrity_digest, third.integrity_digest);
}

#[test]
fn integrity_digest_binds_gate_definition_and_failure_detail() {
    let mut c = candidate("tree-a", "accept-a");
    let runner = FakeGateRunner::passing();
    let findings = FakeFindingsProvider::empty();
    let coverage = FakeCoverageProvider::with(100);
    let baseline = verify(&c, &runner, &findings, &coverage).unwrap();
    c.gates[0].args.push("--strict".into());
    let changed = verify(&c, &runner, &findings, &coverage).unwrap();
    assert_ne!(baseline.integrity_digest, changed.integrity_digest);
}
