//! `retrieve()` idempotency + tie-breaking by id. See BLUEPRINT.md §9.

mod support;

use fleet_memory::{retrieve, Embedding, LexicalHit, MemoryId, ScoreWeights, Timestamp};
use std::cell::Cell;
use std::collections::BTreeMap;
use support::{item, FixedLexical, FixedVector};

#[test]
fn retrieve_is_deterministic_and_ties_break_by_id() {
    let mut items = BTreeMap::new();
    items.insert(MemoryId::parse("z").unwrap(), item("z"));
    items.insert(MemoryId::parse("a").unwrap(), item("a"));
    // Same lexical rank for both ids: their fused `Score` ties, so the sort must break by
    // `MemoryId` ascending rather than leaving insertion/hash order to chance.
    let hits = vec![
        LexicalHit { id: MemoryId::parse("z").unwrap(), bm25_rank: 1 },
        LexicalHit { id: MemoryId::parse("a").unwrap(), bm25_rank: 1 },
    ];
    let lex = FixedLexical(hits, Cell::new(0));
    let vec_ = FixedVector(vec![], Cell::new(0));
    let emb = Embedding::new(vec![1.0]).unwrap();
    let weights = ScoreWeights { alpha: 0.0, beta: 0.0, gamma: 0.0 };
    let out1 = retrieve("q", &emb, &items, Timestamp::from_unix_secs(0), 10, weights, &lex, &vec_).unwrap();
    let out2 = retrieve("q", &emb, &items, Timestamp::from_unix_secs(0), 10, weights, &lex, &vec_).unwrap();
    assert_eq!(out1.len(), 2);
    assert_eq!(out1[0].id.as_str(), "a");
    assert_eq!(out1[1].id.as_str(), "z");
    assert_eq!(
        out1.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
        out2.iter().map(|r| r.id.as_str()).collect::<Vec<_>>()
    );
}
