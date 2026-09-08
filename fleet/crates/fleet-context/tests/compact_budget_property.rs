//! §9 property test: `compact_to_budget` never exceeds its budget, for 50+ seeded random
//! (candidate-size, budget) combinations -- `proptest`'s own seeded RNG, never `thread_rng`.

use fleet_context::{compact_to_budget, ScoredChunk, SymbolId, TokenModel};
use fleet_types::Tokens;
use proptest::prelude::*;

fn chunk(name: &str, text: &str) -> ScoredChunk {
    ScoredChunk {
        id: SymbolId::derive("f.rs", name, 0),
        path: "f.rs".into(),
        text: text.into(),
        score: 1.0,
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn compact_never_exceeds_budget(
        sizes in prop::collection::vec(1u32..40, 0..20),
        budget in 0u64..200,
    ) {
        let candidates: Vec<ScoredChunk> = sizes
            .iter()
            .enumerate()
            .map(|(i, n)| chunk(&format!("c{i}"), &"w ".repeat(*n as usize)))
            .collect();
        let slice = compact_to_budget(candidates, Tokens::new(budget), TokenModel::Cl100kBase, None)
            .unwrap();
        prop_assert!(slice.tokens_used.get() <= slice.tokens_budget.get());
    }
}
