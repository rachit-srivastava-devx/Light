use context::{
    budget::BudgetPacker, compile, manifest::assemble_manifest,
    CompileInput, ContextError, Evidence, RetrievalQuery, Retriever, Span, TokenCounter,
};

struct Tok;
impl TokenCounter for Tok {
    fn count(&self, t: &str) -> u64 { (t.len() as u64).div_ceil(4) }
}

struct Fixed(Vec<Evidence>, String);
impl Retriever for Fixed {
    fn retrieve(&self, _: &RetrievalQuery) -> Result<Vec<Evidence>, ContextError> { Ok(self.0.clone()) }
    fn indexed_base(&self) -> &str { &self.1 }
}

fn ev(id: &str, text: &str) -> Evidence {
    Evidence { source_digest: id.into(), start: 0, end: 1, text: text.into(), trust: 1.0 }
}

fn sp(text: String) -> Span { Span { source_digest: "s".into(), start: 0, end: 1, text } }

// Kills: budget.rs line 30 `>` → `==` and `>` → `>=`
#[test]
fn pack_mandatory_exact_budget_fits() {
    let mut p = BudgetPacker::new(100, 0).unwrap();
    p.pack_mandatory(&sp("a".repeat(400)), &Tok).expect("100 tokens must fit exactly");
}

// Kills: budget.rs line 25 stub (pack_mandatory body → Ok(()))
#[test]
fn pack_mandatory_tracks_usage() {
    let mut p = BudgetPacker::new(5, 0).unwrap(); // 5 tokens effective
    p.pack_mandatory(&sp("a".repeat(16)), &Tok).unwrap(); // 4 tokens consumed, 1 remaining
    assert!(p.try_pack_evidence(&ev("e", "aaaaa"), &Tok).is_none()); // 2 tokens > 1 remaining
}

// Kills: budget.rs line 43 `>` → `==` and `>` → `>=`
#[test]
fn try_pack_evidence_exact_available_fits() {
    let mut p = BudgetPacker::new(4, 0).unwrap(); // available = 4 tokens exactly
    assert!(p.try_pack_evidence(&ev("e", &"a".repeat(16)), &Tok).is_some()); // 4 == available: > false → fits; >= mutation → None
}

// Kills: compile.rs line 14 `||` → `&&`
#[test]
fn compile_empty_plan_digest_rejected() {
    let i = CompileInput { task_digest: "t1".into(), plan_digest: "".into(),
        base_digest: "b1".into(), mandatory: vec![], budget: 100, reserve: 0 };
    assert!(matches!(compile(&i, &Fixed(vec![ev("e", "hi")], "b1".into()), &Tok),
        Err(ContextError::InvalidInput(_))));
}

// Kills: compile.rs line 15 `||` → `&&`
#[test]
fn compile_empty_base_digest_rejected() {
    let i = CompileInput { task_digest: "t1".into(), plan_digest: "p1".into(),
        base_digest: "".into(), mandatory: vec![], budget: 100, reserve: 0 };
    assert!(matches!(compile(&i, &Fixed(vec![ev("e", "hi")], "b1".into()), &Tok),
        Err(ContextError::InvalidInput(_))));
}

// Kills: compile.rs line 50 `|| checked == 0` → `&& checked == 0`
#[test]
fn compile_all_evidence_omitted_is_zero_coverage() {
    let i = CompileInput { task_digest: "t1".into(), plan_digest: "p1".into(),
        base_digest: "b1".into(), mandatory: vec![sp("a".repeat(16))], budget: 4, reserve: 0 };
    assert!(matches!(compile(&i, &Fixed(vec![ev("e", "a")], "b1".into()), &Tok),
        Err(ContextError::ZeroCoverage)));
}

// Kills: manifest.rs line 23 `^ 0xff` → `| 0xff` (→ "df1449d88bcfeeff") and `& 0xff` (→ "00000000000000b0")
#[test]
fn fnv1a_exact_digest_is_pinned() {
    let m = assemble_manifest("t1", "p1", "b1", vec![], vec![], vec![], 0, 0);
    assert_eq!(m.digest, "204daa2beb207d27");
}
