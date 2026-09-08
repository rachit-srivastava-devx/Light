use fleet_plan::{derive_lesson, validate_challenge_rows, ChallengeRow, LessonSource, TaughtOutcome};
use fleet_types::{NodeId, Role};
use std::collections::BTreeSet;

#[test]
fn derive_lesson_from_gate_refusal_is_valid_challenge_input() {
    let outcome = TaughtOutcome::GateRefused { check_id: "C1-OPEN", detail: "open_questions non-empty".into() };
    let leaf = NodeId::parse("some-leaf").unwrap();
    let lesson = derive_lesson(&outcome, LessonSource::FailureCorpus("gate-refusal".into()), leaf.clone(), Role::Builder);

    let mut atomic_ids = BTreeSet::new();
    atomic_ids.insert(leaf.as_str().to_string());
    let row = ChallengeRow {
        id: "c1".into(),
        source: lesson.source.as_wire_ref(),
        affected_leaf: leaf.as_str().to_string(),
        risk: lesson.risk.clone(),
        trigger: lesson.trigger.clone(),
        mitigation: lesson.mitigation.clone(),
    };
    let violations = validate_challenge_rows(&[row], &atomic_ids, |s| s == "FAILURE-CORPUS:gate-refusal");
    assert!(violations.is_empty(), "{violations:?}");
}

#[test]
fn lesson_source_wire_ref_matches_the_two_recognised_prefixes() {
    assert_eq!(LessonSource::FailureCorpus("k".into()).as_wire_ref(), "FAILURE-CORPUS:k");
    assert_eq!(LessonSource::ThreadLessons("k".into()).as_wire_ref(), "THREAD-LESSONS:k");
}

#[test]
fn derive_lesson_never_produces_an_empty_mitigation_for_no_killer() {
    let outcome = TaughtOutcome::MutationSurvived { mutant: "m1".into(), killed_by: None };
    let leaf = NodeId::parse("leaf-x").unwrap();
    let lesson = derive_lesson(&outcome, LessonSource::ThreadLessons("t".into()), leaf, Role::Verifier);
    assert!(!lesson.mitigation.is_empty());
    assert!(lesson.mitigation.contains("no test"));
}

#[test]
fn derive_lesson_is_pure_and_idempotent() {
    let outcome = TaughtOutcome::Rejected { reviewer_role: "verifier".into(), reason: "flaky test".into() };
    let leaf = NodeId::parse("leaf-y").unwrap();
    let a = derive_lesson(&outcome, LessonSource::FailureCorpus("x".into()), leaf.clone(), Role::Builder);
    let b = derive_lesson(&outcome, LessonSource::FailureCorpus("x".into()), leaf, Role::Builder);
    assert_eq!(a, b);
}
