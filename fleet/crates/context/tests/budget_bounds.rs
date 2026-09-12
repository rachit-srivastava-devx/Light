use context::{budget::pack, ContextError, Evidence, EvidenceRef, Span, TokenCounter};

struct Tok;
impl TokenCounter for Tok {
    fn count(&self, t: &str) -> u64 { (t.len() as u64).div_ceil(4) }
}

fn ev(id: &str, content: &str) -> Evidence {
    Evidence { source_digest: id.into(), content: content.into(), tokens: 0 }
}

fn sp(label: &str) -> Span {
    Span { source_digest: "s".into(), start: 0, end: 1, label: label.into() }
}

// Kills: budget.rs `>` → `==` and `>` → `>=` for mandatory overflow check
#[test]
fn mandatory_exact_budget_fits() {
    let mandatory = vec![sp(&"a".repeat(400))]; // 100 tokens exactly
    let result = pack(&mandatory, &[], 100, 0, &Tok);
    assert!(result.is_ok(), "100 tokens must fit in budget=100, got {:?}", result);
}

// Kills: budget.rs mandatory overflow check — mandatory exceeds budget
#[test]
fn mandatory_overflow_rejected() {
    let mandatory = vec![sp(&"a".repeat(408))]; // 102 tokens
    let result = pack(&mandatory, &[], 100, 0, &Tok);
    assert!(matches!(result, Err(ContextError::BudgetExceeded)));
}

// Kills: evidence inclusion `>` → `>=` (new_used <= available)
#[test]
fn evidence_exact_available_fits() {
    let mandatory = vec![sp("")]; // 0 tokens mandatory
    let candidates = vec![ev("e", &"a".repeat(16))]; // 4 tokens
    let result = pack(&mandatory, &candidates, 4, 0, &Tok).unwrap();
    assert_eq!(result.0.len(), 1, "4 tokens should fit exactly in budget=4");
}

// Kills: evidence inclusion off-by-one
#[test]
fn evidence_overflow_omitted() {
    let mandatory = vec![sp(&"a".repeat(4))]; // 1 token mandatory
    let candidates = vec![ev("e", &"a".repeat(16))]; // 4 tokens, only 3 left
    let result = pack(&mandatory, &candidates, 4, 0, &Tok).unwrap();
    assert_eq!(result.0.len(), 0, "evidence must be omitted when it doesn't fit");
    assert_eq!(result.1, vec!["e".to_string()]);
}

// Kills: dedup removed (inserting same source twice)
#[test]
fn duplicate_evidence_deduped() {
    let candidates = vec![ev("same", "hi"), ev("same", "hi")];
    let result = pack(&[], &candidates, 100, 0, &Tok).unwrap();
    assert_eq!(result.0.len(), 1, "duplicates must be deduplicated");
}

// Kills: reserve not subtracted (reserve > 0 reduces available)
#[test]
fn reserve_reduces_available() {
    let candidates = vec![ev("e", &"a".repeat(16))]; // 4 tokens
    let result = pack(&[], &candidates, 5, 2, &Tok).unwrap(); // available = 3
    assert_eq!(result.0.len(), 0, "reserve=2 leaves 3 tokens; 4 token item must be omitted");
}

// Kills: missing provenance check removed
#[test]
fn empty_source_digest_rejected() {
    let candidates = vec![ev("", "content")];
    let result = pack(&[], &candidates, 100, 0, &Tok);
    assert!(matches!(result, Err(ContextError::MissingProvenance)));
}
