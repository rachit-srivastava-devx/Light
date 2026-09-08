//! Shared test doubles/fixtures for `fleet-memory`'s integration tests. `#![allow(dead_code)]`
//! because each `tests/*.rs` binary is compiled separately and uses only a subset of these.

#![allow(dead_code)]

use fleet_memory::{Embedding, Importance, MemoryId, MemoryItem, MemoryKind, Timestamp};
use std::cell::Cell;

pub fn item(id: &str) -> MemoryItem {
    MemoryItem {
        id: MemoryId::parse(id).unwrap(),
        kind: MemoryKind::Semantic,
        text: "x".into(),
        embedding: Embedding::new(vec![1.0]).unwrap(),
        importance: Importance::new(0.0).unwrap(),
        created_at: Timestamp::from_unix_secs(0),
        last_confirmed_at: Timestamp::from_unix_secs(0),
        confirmed_count: 0,
    }
}

pub struct FixedLexical(pub Vec<fleet_memory::LexicalHit>, pub Cell<u32>);
impl fleet_memory::LexicalSearch for FixedLexical {
    fn search(&self, _q: &str, _l: usize) -> Result<Vec<fleet_memory::LexicalHit>, fleet_memory::RetrieveError> {
        self.1.set(self.1.get() + 1);
        Ok(self.0.clone())
    }
}
pub struct FixedVector(pub Vec<fleet_memory::VectorHit>, pub Cell<u32>);
impl fleet_memory::VectorSearch for FixedVector {
    fn knn(&self, _e: &Embedding, _l: usize) -> Result<Vec<fleet_memory::VectorHit>, fleet_memory::RetrieveError> {
        self.1.set(self.1.get() + 1);
        Ok(self.0.clone())
    }
}
