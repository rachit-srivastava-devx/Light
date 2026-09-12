use std::collections::HashMap;

use crate::types::{CandidateError, CandidateLesson};

/// Store port: atomic insert and receipt as one durable operation.
pub trait CandidateStore {
    fn insert(&mut self, c: &CandidateLesson) -> Result<(), CandidateError>;
}

/// In-memory store for testing and single-process use.
pub struct InMemoryStore {
    seen: HashMap<String, String>,
}

impl InMemoryStore {
    pub fn new() -> Self {
        Self {
            seen: HashMap::new(),
        }
    }
}

impl Default for InMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl CandidateStore for InMemoryStore {
    fn insert(&mut self, c: &CandidateLesson) -> Result<(), CandidateError> {
        if self.seen.contains_key(&c.id) {
            return Err(CandidateError::Conflict);
        }
        self.seen.insert(c.id.clone(), c.fixture_digest.clone());
        Ok(())
    }
}
