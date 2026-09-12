use candidate::{build, CandidateError, CandidateStore, Evidence, InMemoryStore};

fn make_evidence() -> Evidence {
    Evidence {
        task_type: "verify".to_string(),
        signature: "sig-abc".to_string(),
        tree_digest: "tree-xyz".to_string(),
        evidence_digest: "evd-xyz".to_string(),
        checked: 3,
        total: 3,
    }
}
#[test]
fn id_format_is_type_colon_tree_colon_evidence() {
    let lesson = build(make_evidence(), "fix".to_string()).unwrap();
    assert_eq!(lesson.id, "verify:tree-xyz:evd-xyz");
}
#[test]
fn scope_equals_task_type() {
    let lesson = build(make_evidence(), "fix".to_string()).unwrap();
    assert_eq!(lesson.scope, "verify");
}
#[test]
fn provenance_is_exactly_one_tree_digest() {
    let lesson = build(make_evidence(), "fix".to_string()).unwrap();
    assert_eq!(lesson.provenance, vec!["tree-xyz".to_string()]);
}
#[test]
fn fixture_digest_preserved_exactly() {
    let lesson = build(make_evidence(), "exact-digest".to_string()).unwrap();
    assert_eq!(lesson.fixture_digest, "exact-digest");
}
#[test]
fn partial_coverage_refused() {
    let e = Evidence {
        task_type: "t".into(),
        signature: "s".into(),
        tree_digest: "d".into(),
        evidence_digest: "e".into(),
        checked: 1,
        total: 2,
    };
    assert!(matches!(build(e, "fx".into()), Err(CandidateError::Coverage)));
}
#[test]
fn empty_task_type_is_invalid() {
    let mut e = make_evidence();
    e.task_type = String::new();
    assert!(matches!(build(e, "fx".into()), Err(CandidateError::Invalid)));
}
#[test]
fn empty_evidence_digest_is_invalid() {
    let mut e = make_evidence();
    e.evidence_digest = String::new();
    assert!(matches!(build(e, "fx".into()), Err(CandidateError::Invalid)));
}
#[test]
fn empty_fixture_digest_is_invalid() {
    assert!(matches!(build(make_evidence(), String::new()), Err(CandidateError::Invalid)));
}
#[test]
fn empty_tree_digest_is_invalid() {
    let mut e = make_evidence();
    e.tree_digest = String::new();
    assert!(matches!(build(e, "fx".into()), Err(CandidateError::Invalid)));
}
#[test]
fn empty_signature_is_invalid() {
    let mut e = make_evidence();
    e.signature = String::new();
    assert!(matches!(build(e, "fx".into()), Err(CandidateError::Invalid)));
}
#[test]
fn in_memory_store_insert_succeeds_once() {
    let lesson = build(make_evidence(), "fix".to_string()).unwrap();
    let mut store = InMemoryStore::new();
    assert!(store.insert(&lesson).is_ok());
}
