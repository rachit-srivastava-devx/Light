use super::*;

#[test]
fn accepts_the_inclusive_maximum_floor() {
    let mut c = candidate(vec![gate("g1")]);
    c.coverage_floor = Some(100);
    let result = verify(
        &c,
        &FakeGateRunner::passing(),
        &FakeFindingsProvider::empty(),
        &FakeCoverageProvider::with(100),
    );
    assert!(result.is_ok(), "floor=100 must be valid: {result:?}");
}
