use review::{
    assemble_reviewed_candidate, CandidateObservation, ContextManifest, Finding, FindingsProvider,
};

struct MockSemgrep { findings: Vec<Finding> }
impl FindingsProvider for MockSemgrep {
    fn findings(&self, _: &str) -> Vec<Finding> { self.findings.clone() }
}

fn load_fixture() -> CandidateObservation {
    let s = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/blueprint-review/immutable-diff.json")).expect("fixture");
    serde_json::from_str(&s).expect("fixture must parse as CandidateObservation")
}
#[test]
fn candidate_observation_triggers_code_review() {
    let obs = load_fixture();
    let mock = MockSemgrep { findings: vec![Finding { severity: "INFO".into(), path: "src/findings.rs".into(), rationale: "test finding".into() }] };
    let candidate = assemble_reviewed_candidate(&obs, None, Box::new(mock)).expect("review must succeed");
    assert!(!candidate.result.findings.is_empty(), "must have at least one finding");
    assert!(!candidate.result.input_digest.is_empty(), "input_digest must be non-empty");
    assert!(!candidate.result.output_digest.is_empty(), "output_digest must be non-empty");
}
#[test]
fn semgrep_finding_marks_review_failed() {
    let obs = load_fixture();
    let mock = MockSemgrep { findings: vec![Finding { severity: "WARNING".into(), path: "src/findings.rs".into(), rationale: "potential issue detected".into() }] };
    let candidate = assemble_reviewed_candidate(&obs, None, Box::new(mock)).expect("review must succeed");
    assert!(!candidate.passed, "WARNING finding must set passed = false");
}
#[test]
fn context_manifest_scopes_review_evidence() {
    let obs = load_fixture();
    let manifest = ContextManifest { scope: vec!["src/findings.rs".to_string()] };
    let mock = MockSemgrep { findings: vec![
        Finding { severity: "INFO".into(), path: "src/findings.rs".into(), rationale: "findings file finding".into() },
        Finding { severity: "INFO".into(), path: "src/decision.rs".into(), rationale: "decision file finding".into() },
    ]};
    let candidate = assemble_reviewed_candidate(&obs, Some(&manifest), Box::new(mock)).expect("review must succeed");
    assert!(!candidate.result.findings.is_empty(), "scoped findings must be non-empty");
    for f in &candidate.result.findings {
        assert_eq!(f.path, "src/findings.rs", "only src/findings.rs findings must survive scope filter");
    }
}
