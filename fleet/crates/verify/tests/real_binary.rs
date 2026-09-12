// Integration smoke test: confirm the verify crate is importable and the
// public API types are accessible. The five named acceptance tests live in
// src/tests.rs under the `verify::tests` module to preserve the canonical
// `verify::tests::*` test paths required by the blueprint §10.

#[test]
fn verify_crate_public_api_accessible() {
    let spec = verify::CanonicalGateSpec {
        id: "smoke".into(),
        command: "true".into(),
        args: vec![],
    };
    let candidate = verify::ReviewedCandidate {
        tree_digest: "d1".into(),
        acceptance_digest: "d2".into(),
        gates: vec![spec],
        coverage_floor: None,
        output_digest: None,
    };
    let ev = verify::verify(
        &candidate,
        &verify::FakeGateRunner::passing(),
        &verify::FakeFindingsProvider::empty(),
        &verify::FakeCoverageProvider::with(100.0),
    )
    .unwrap();
    assert!(ev.passed);
    assert_eq!(ev.checked, ev.total);
}
