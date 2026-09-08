//! `retrieve()` RRF fusion + fail-fast port errors. See BLUEPRINT.md §9.

mod support;

use fleet_memory::{retrieve, Embedding, LexicalHit, LexicalSearch, MemoryId, MemoryItem, RetrieveError, ScoreWeights, Timestamp, VectorHit, VectorSearch};
use std::cell::Cell;
use std::collections::BTreeMap;
use support::{item, FixedLexical, FixedVector};

struct FailingLexical;
impl LexicalSearch for FailingLexical {
    fn search(&self, _q: &str, _l: usize) -> Result<Vec<LexicalHit>, RetrieveError> {
        Err(RetrieveError("boom".into()))
    }
}
struct CountingVector(Cell<u32>);
impl VectorSearch for CountingVector {
    fn knn(&self, _e: &Embedding, _l: usize) -> Result<Vec<VectorHit>, RetrieveError> {
        self.0.set(self.0.get() + 1);
        Ok(vec![])
    }
}

#[test]
fn retrieve_fuses_lexical_and_vector_hits_via_rrf() {
    let mut items = BTreeMap::new();
    items.insert(MemoryId::parse("a").unwrap(), item("a"));
    items.insert(MemoryId::parse("b").unwrap(), item("b"));
    let lex = FixedLexical(vec![LexicalHit { id: MemoryId::parse("a").unwrap(), bm25_rank: 2 }], Cell::new(0));
    let vec_ = FixedVector(
        vec![
            VectorHit { id: MemoryId::parse("a").unwrap(), vector_rank: 1, distance: 0.1 },
            VectorHit { id: MemoryId::parse("b").unwrap(), vector_rank: 3, distance: 0.2 },
        ],
        Cell::new(0),
    );
    let emb = Embedding::new(vec![1.0]).unwrap();
    let weights = ScoreWeights { alpha: 0.0, beta: 0.0, gamma: 1.0 };
    let out = retrieve("q", &emb, &items, Timestamp::from_unix_secs(0), 10, weights, &lex, &vec_).unwrap();
    let a = out.iter().find(|r| r.id.as_str() == "a").unwrap();
    assert!((a.relevance.get() - (1.0 / 62.0 + 1.0 / 61.0)).abs() < 1e-9);
    let b = out.iter().find(|r| r.id.as_str() == "b").unwrap();
    assert!((b.relevance.get() - 1.0 / 63.0).abs() < 1e-9);
}

#[test]
fn retrieve_fails_fast_on_first_port_error() {
    let items: BTreeMap<MemoryId, MemoryItem> = BTreeMap::new();
    let lex = FailingLexical;
    let vec_ = CountingVector(Cell::new(0));
    let emb = Embedding::new(vec![1.0]).unwrap();
    let weights = ScoreWeights { alpha: 0.0, beta: 0.0, gamma: 0.0 };
    let err = retrieve("q", &emb, &items, Timestamp::from_unix_secs(0), 10, weights, &lex, &vec_).unwrap_err();
    assert_eq!(err, RetrieveError("boom".into()));
    assert_eq!(vec_.0.get(), 0, "vector.knn must not be called after lexical.search fails");
}
