use context::{
    compile, CompileInput, ContextError, ContextManifest,
    Evidence, RetrievalQuery, Retriever, Span, TokenCounter,
};

struct Tok;
impl TokenCounter for Tok {
    fn count(&self, t: &str) -> u64 { (t.len() as u64).div_ceil(4) }
}

struct Fixed { digest: String, items: Vec<Evidence> }
impl Retriever for Fixed {
    fn retrieve(&self, _: &RetrievalQuery) -> Result<Vec<Evidence>, ContextError> { Ok(self.items.clone()) }
    fn index_digest(&self) -> &str { &self.digest }
}

fn ev(id: &str) -> Evidence {
    Evidence { source_digest: id.into(), content: format!("content-{id}"), tokens: 0 }
}

fn mandatory_span(id: &str) -> Span {
    Span { source_digest: id.into(), start: 0, end: 10, label: "mandatory".into() }
}

fn input(base: &str) -> CompileInput {
    CompileInput {
        task_digest: "t1".into(),
        plan_digest: "p1".into(),
        base_digest: base.into(),
        mandatory: vec![mandatory_span("m1")],
        budget: 1000,
        reserve: 100,
    }
}

#[test]
fn compile_produces_manifest_with_checked_total() {
    let r = Fixed { digest: "b1".into(), items: vec![ev("ev1"), ev("ev2")] };
    let m = compile(&input("b1"), &r, &Tok).expect("compile should succeed");
    assert!(m.checked > 0, "checked must be nonzero");
    assert!(m.total > 0, "total must be nonzero");
    assert!(!m.digest.is_empty(), "digest must be non-empty");
    assert_eq!(m.mandatory.len(), 1);
}

#[test]
fn manifest_is_json_roundtrippable() {
    let r = Fixed { digest: "b1".into(), items: vec![ev("ev1")] };
    let manifest = compile(&input("b1"), &r, &Tok).expect("compile");
    let json = serde_json::to_string(&manifest).expect("serialize");
    let roundtrip: ContextManifest = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(manifest, roundtrip);
}

// Kills: simple_digest → constant "xyzzy" (two different evidence sets → same digest)
#[test]
fn digest_changes_when_evidence_changes() {
    let r1 = Fixed { digest: "b1".into(), items: vec![ev("ev-A")] };
    let r2 = Fixed { digest: "b1".into(), items: vec![ev("ev-B")] };
    let m1 = compile(&input("b1"), &r1, &Tok).unwrap();
    let m2 = compile(&input("b1"), &r2, &Tok).unwrap();
    assert_ne!(m1.digest, m2.digest, "digest must change when evidence changes");
    assert_eq!(m1.digest.len(), 16, "digest must be a 16-char hex string");
}

// Kills: stale guard removed — base_digest mismatch must return Stale
#[test]
fn stale_index_refused() {
    let r = Fixed { digest: "b-real".into(), items: vec![ev("ev1")] };
    let inp = CompileInput { base_digest: "b-mismatch".into(), ..input("b-mismatch") };
    let result = compile(&inp, &r, &Tok);
    assert!(
        matches!(result, Err(ContextError::Stale { .. })),
        "expected Stale, got {:?}", result,
    );
}

// Kills: empty-mandatory guard removed
#[test]
fn empty_mandatory_refused() {
    let r = Fixed { digest: "b1".into(), items: vec![ev("ev1")] };
    let inp = CompileInput { mandatory: vec![], ..input("b1") };
    let result = compile(&inp, &r, &Tok);
    assert!(matches!(result, Err(ContextError::ZeroCoverage)));
}
