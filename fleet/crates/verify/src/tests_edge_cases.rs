use super::*;

#[test]
fn coverage_below_floor_marks_gate_failed() {
    let mut c = candidate(vec![spec("g1")]);
    c.coverage_floor = Some(80);
    let ev = run(&c, true, 70, FakeFindingsProvider::empty());
    let cov = ev.gate_results.iter().find(|r| r.id == "coverage").unwrap();
    assert!(!cov.passed);
    let msg = cov.failure_message.as_deref().unwrap_or("");
    assert!(msg.contains("70"), "msg={msg}");
    assert!(msg.contains("80"), "msg={msg}");
}

#[test]
fn secret_finding_blocks_gate() {
    let ev = run(
        &candidate(vec![spec("g1")]),
        true,
        100,
        FakeFindingsProvider::with_critical("secrets.txt"),
    );
    assert!(!ev.passed);
    assert!(!ev.findings.is_empty());
}

#[test]
fn evidence_digest_mismatch_refused() {
    let mut c = candidate(vec![spec("g1")]);
    c.output_digest = Some("wrong-digest".into());
    let ev = run(&c, true, 100, FakeFindingsProvider::empty());
    assert!(!ev.passed);
    let found = ev
        .failures
        .iter()
        .any(|f| f.contains("mismatch") || f.contains("digest"));
    assert!(found, "failures: {:?}", ev.failures);
}

#[test]
fn empty_evidence_cannot_pass_with_zero_denominator() {
    let ev = crate::assemble_gate_evidence(&candidate(vec![spec("g1")]), vec![], vec![]);
    assert_eq!(ev.checked, 0);
    assert_eq!(ev.total, 0);
    assert!(!ev.passed);
    assert!(ev.failures.iter().any(|f| f.contains("no gate inputs")));
}
