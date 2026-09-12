use review::{
    assemble_reviewed_candidate, CandidateObservation, Decision, Finding,
    FindingsProvider, ReviewError,
};
use review::findings::{parse_ruff_findings, parse_semgrep_findings};

struct Fixed(Vec<Finding>);
impl FindingsProvider for Fixed {
    fn findings(&self, _: &str) -> Vec<Finding> { self.0.clone() }
}

fn obs(diff: &str, paths: &[&str], w: &str, r: &str) -> CandidateObservation {
    CandidateObservation { diff: diff.into(), changed_paths: paths.iter().map(|s| s.to_string()).collect(), worker_id: w.into(), reviewer_id: r.into(), tree_digest: "t".into(), plan_digest: "p".into(), acceptance_digest: "a".into() }
}

fn f(sev: &str) -> Finding {
    Finding { severity: sev.into(), path: "f.rs".into(), rationale: "r".into() }
}
#[test]
fn same_worker_reviewer_is_not_independent() {
    let e = assemble_reviewed_candidate(&obs("d", &[], "same", "same"), None,
        Box::new(Fixed(vec![]))).unwrap_err();
    assert!(matches!(e, ReviewError::NotIndependent));
}
#[test]
fn info_findings_pass_and_decision_is_approved() {
    let r = assemble_reviewed_candidate(&obs("d", &["f"], "w", "rev"), None,
        Box::new(Fixed(vec![f("INFO")]))).unwrap();
    assert!(r.passed, "INFO must not fail review");
    assert!(matches!(r.result.decision, Decision::Approved));
}
#[test]
fn error_severity_fails_and_changes_requested() {
    let r = assemble_reviewed_candidate(&obs("d", &["f"], "w", "rev"), None,
        Box::new(Fixed(vec![f("ERROR")]))).unwrap();
    assert!(!r.passed, "ERROR must fail review");
    assert!(matches!(r.result.decision, Decision::ChangesRequested));
}
#[test]
fn input_digest_is_16_hex_chars() {
    let r = assemble_reviewed_candidate(&obs("d", &[], "w", "r"), None,
        Box::new(Fixed(vec![]))).unwrap();
    assert_eq!(r.result.input_digest.len(), 16, "input_digest must be 16 chars");
    assert!(r.result.input_digest.chars().all(|c| c.is_ascii_hexdigit()), "must be hex");
}
#[test]
fn output_digest_is_16_hex_chars() {
    let r = assemble_reviewed_candidate(&obs("d", &[], "w", "r"), None,
        Box::new(Fixed(vec![]))).unwrap();
    assert_eq!(r.result.output_digest.len(), 16, "output_digest must be 16 chars");
    assert!(r.result.output_digest.chars().all(|c| c.is_ascii_hexdigit()), "must be hex");
}
#[test]
fn parse_semgrep_findings_extracts_result() {
    let json = serde_json::json!({
        "results": [{"extra": {"severity": "WARNING", "message": "issue"}, "path": "src/lib.rs"}]
    });
    let findings = parse_semgrep_findings(&json).unwrap();
    assert_eq!(findings.len(), 1, "must extract one finding");
    assert_eq!(findings[0].severity, "WARNING");
    assert_eq!(findings[0].path, "src/lib.rs");
}
#[test]
fn parse_ruff_findings_extracts_result() {
    let json = serde_json::json!([{"filename": "src/main.rs", "message": "unused import"}]);
    let findings = parse_ruff_findings(&json).unwrap();
    assert_eq!(findings.len(), 1, "must extract one finding");
    assert_eq!(findings[0].severity, "WARNING");
    assert_eq!(findings[0].path, "src/main.rs");
}
