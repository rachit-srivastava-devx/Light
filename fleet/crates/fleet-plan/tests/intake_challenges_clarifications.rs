use fleet_plan::{validate_challenge_rows, validate_clarification_rows, ChallengeRow, ClarificationKind, ClarificationRow};
use std::collections::BTreeSet;

#[test]
fn challenge_row_rejects_unknown_corpus_source() {
    let mut atomic_ids = BTreeSet::new();
    atomic_ids.insert("f1".to_string());
    let rows = vec![ChallengeRow {
        id: "c1".into(),
        source: "FAILURE-CORPUS:missing".into(),
        affected_leaf: "f1".into(),
        risk: "r".into(),
        trigger: "t".into(),
        mitigation: "m".into(),
    }];
    let v = validate_challenge_rows(&rows, &atomic_ids, |_| false);
    assert!(v.iter().any(|e| e.0.contains("cites no known corpus row")));
}

#[test]
fn challenge_row_accepts_known_source_and_leaf() {
    let mut atomic_ids = BTreeSet::new();
    atomic_ids.insert("f1".to_string());
    let rows = vec![ChallengeRow {
        id: "c1".into(),
        source: "FAILURE-CORPUS:known".into(),
        affected_leaf: "f1".into(),
        risk: "r".into(),
        trigger: "t".into(),
        mitigation: "m".into(),
    }];
    let v = validate_challenge_rows(&rows, &atomic_ids, |_| true);
    assert!(v.is_empty(), "{v:?}");
}

#[test]
fn clarification_rejects_generic_question() {
    let rows = vec![ClarificationRow {
        id: "cl1".into(),
        kind: ClarificationKind::Business,
        gap_ref: "sow:foo".into(),
        question: "What do you want here?".into(),
        blocking: true,
        answer: "".into(),
    }];
    let v = validate_clarification_rows(&rows, |_| true);
    assert!(v.iter().any(|e| e.0.contains("generic noise")));
}

#[test]
fn clarification_nonblocking_needs_answer() {
    let rows = vec![ClarificationRow {
        id: "cl1".into(),
        kind: ClarificationKind::Technical,
        gap_ref: "sow:foo".into(),
        question: "specific gap in foo".into(),
        blocking: false,
        answer: "".into(),
    }];
    let v = validate_clarification_rows(&rows, |_| true);
    assert!(v.iter().any(|e| e.0.contains("still needs an answer")));
}
