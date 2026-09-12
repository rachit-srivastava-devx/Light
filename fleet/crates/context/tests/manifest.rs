use context::{
    compile, CompileInput, ContextError, ContextManifest, Evidence,
    RetrievalQuery, Retriever, Span,
};

struct MockCounter;
impl context::TokenCounter for MockCounter {
    fn count(&self, text: &str) -> u64 {
        (text.len() as u64).div_ceil(4)
    }
}

struct MockRetriever {
    indexed_base: String,
    items: Vec<Evidence>,
}

impl Retriever for MockRetriever {
    fn retrieve(&self, _q: &RetrievalQuery) -> Result<Vec<Evidence>, ContextError> {
        Ok(self.items.clone())
    }
    fn indexed_base(&self) -> &str {
        &self.indexed_base
    }
}

fn ev(id: &str) -> Evidence {
    Evidence { source_digest: id.into(), start: 0, end: 5, text: format!("text {id}"), trust: 0.9 }
}

#[test]
fn knowledge_to_context_manifest() {
    let span = Span { source_digest: "abc".into(), start: 0, end: 10, text: "mandatory".into() };
    let input = CompileInput {
        task_digest: "t1".into(),
        plan_digest: "p1".into(),
        base_digest: "b1".into(),
        mandatory: vec![span],
        budget: 1000,
        reserve: 100,
    };
    let r = MockRetriever {
        indexed_base: "b1".into(),
        items: vec![ev("ev1"), ev("ev2")],
    };
    let m = compile(&input, &r, &MockCounter).expect("compile should succeed");
    assert!(m.checked > 0, "checked must be nonzero");
    assert!(m.total > 0, "total must be nonzero");
    assert!(!m.digest.is_empty(), "digest must be non-empty");
    assert_eq!(m.mandatory.len(), 1);
}

#[test]
fn context_to_builder() {
    let span = Span { source_digest: "abc".into(), start: 0, end: 10, text: "mandatory".into() };
    let input = CompileInput {
        task_digest: "t1".into(),
        plan_digest: "p1".into(),
        base_digest: "b1".into(),
        mandatory: vec![span],
        budget: 1000,
        reserve: 100,
    };
    let r = MockRetriever { indexed_base: "b1".into(), items: vec![ev("ev1")] };
    let manifest = compile(&input, &r, &MockCounter).expect("compile should succeed");
    let json = serde_json::to_string(&manifest).expect("serialize");
    let roundtrip: ContextManifest = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(manifest, roundtrip);
    assert!(!roundtrip.digest.is_empty());
}

#[test]
fn digest_changes_when_input_changes() {
    // Kills: `compute_digest -> "xyzzy"` (both calls return "xyzzy", equal)
    // Kills: `^= with &=` in fnv1a (hash degrades, both inputs collapse to same value)
    // Kills: `^ 0xff with | 0xff` and `& 0xff` (alters mixing, breaks uniqueness)
    let span = Span { source_digest: "abc".into(), start: 0, end: 10, text: "m".into() };
    let base = CompileInput {
        task_digest: "task-A".into(),
        plan_digest: "plan-1".into(),
        base_digest: "b1".into(),
        mandatory: vec![span.clone()],
        budget: 1000,
        reserve: 100,
    };
    let r1 = MockRetriever { indexed_base: "b1".into(), items: vec![ev("ev1")] };
    let m1 = compile(&base, &r1, &MockCounter).unwrap();

    let changed = CompileInput { task_digest: "task-B".into(), ..base };
    let r2 = MockRetriever { indexed_base: "b1".into(), items: vec![ev("ev1")] };
    let m2 = compile(&changed, &r2, &MockCounter).unwrap();

    assert_ne!(m1.digest, m2.digest, "digest must change when task_digest changes");
    assert_eq!(m1.digest.len(), 16, "digest must be a 16-char hex string");
}

#[test]
fn stale_knowledge_ref_refused() {
    let input = CompileInput {
        task_digest: "t1".into(),
        plan_digest: "p1".into(),
        base_digest: "b-mismatch".into(),
        mandatory: vec![],
        budget: 1000,
        reserve: 100,
    };
    let r = MockRetriever { indexed_base: "b-real".into(), items: vec![ev("ev1")] };
    let result = compile(&input, &r, &MockCounter);
    assert!(
        matches!(result, Err(ContextError::Stale)),
        "expected Stale, got {result:?}",
    );
}
