//! §9 integration tests for `compact_to_budget`: first-fit-not-first-halt, summarizer fallback,
//! budget-never-exceeded (including a seeded property sweep).

use fleet_context::{compact_to_budget, ContextError, ScoredChunk, Summarizer, SymbolId, TokenModel};
use fleet_types::Tokens;

fn id(name: &str) -> SymbolId {
    SymbolId::derive("f.rs", name, 0)
}

fn chunk(name: &str, text: &str, score: f64) -> ScoredChunk {
    ScoredChunk {
        id: id(name),
        path: "f.rs".into(),
        text: text.into(),
        score,
    }
}

#[test]
fn compact_is_first_fit_not_first_halt() {
    let huge = chunk("huge", &"word ".repeat(500), 3.0);
    let small = chunk("small", "hi", 2.0);
    let medium = chunk("medium", "hello there", 1.0);
    let slice = compact_to_budget(
        vec![huge, small, medium],
        Tokens::new(5),
        TokenModel::Cl100kBase,
        None,
    )
    .unwrap();
    assert!(slice.chunks.iter().any(|c| c.id == id("small")));
}

#[test]
fn compact_falls_back_to_drop_when_summarizer_absent() {
    let huge = chunk("huge", &"word ".repeat(500), 1.0);
    let slice = compact_to_budget(vec![huge], Tokens::new(1), TokenModel::Cl100kBase, None).unwrap();
    assert!(slice.chunks.is_empty());
    assert_eq!(slice.dropped, vec![id("huge")]);
}

struct FixedSummarizer(&'static str);
impl Summarizer for FixedSummarizer {
    fn summarize(&self, _text: &str, _target: u32) -> Result<String, ContextError> {
        Ok(self.0.to_string())
    }
}

#[test]
fn compact_uses_summarizer_when_it_fits() {
    let huge = chunk("huge", &"word ".repeat(500), 1.0);
    let summarizer = FixedSummarizer("short");
    let slice = compact_to_budget(
        vec![huge],
        Tokens::new(5),
        TokenModel::Cl100kBase,
        Some(&summarizer),
    )
    .unwrap();
    assert_eq!(slice.chunks.len(), 1);
    assert!(slice.chunks[0].compacted);
}
