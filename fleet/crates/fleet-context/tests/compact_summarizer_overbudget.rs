//! §9: a `Summarizer` that oversells its own output (returns text whose real token count exceeds
//! the requested target) must abort the whole call with `SummarizeOverBudget`, not be truncated.

use fleet_context::{compact_to_budget, ContextError, ScoredChunk, Summarizer, SymbolId, TokenModel};
use fleet_types::Tokens;

struct OverselllingSummarizer;
impl Summarizer for OverselllingSummarizer {
    fn summarize(&self, _text: &str, _target: u32) -> Result<String, ContextError> {
        Ok("word ".repeat(50))
    }
}

#[test]
fn compact_rejects_a_summarizer_that_oversells_its_output() {
    let huge = ScoredChunk {
        id: SymbolId::derive("f.rs", "huge", 0),
        path: "f.rs".into(),
        text: "word ".repeat(500),
        score: 1.0,
    };
    let summarizer = OverselllingSummarizer;
    let result = compact_to_budget(
        vec![huge],
        Tokens::new(5),
        TokenModel::Cl100kBase,
        Some(&summarizer),
    );
    assert!(matches!(result, Err(ContextError::SummarizeOverBudget { .. })));
}
