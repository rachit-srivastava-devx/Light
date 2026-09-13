use std::collections::HashMap;
use std::sync::Mutex;

use crate::filter::filter_by_scope;
use crate::promote::promote_candidate;
use crate::{KnowledgeError, KnowledgeItem, KnowledgeStore, Scope};

/// Thread-safe in-memory implementation of [`KnowledgeStore`].
pub struct InMemoryStore {
    items: Mutex<HashMap<String, KnowledgeItem>>,
}

impl InMemoryStore {
    pub fn new() -> Self {
        Self {
            items: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl KnowledgeStore for InMemoryStore {
    fn list(
        &self,
        query: &str,
        scope: &Scope,
        limit: u32,
    ) -> Result<Vec<KnowledgeItem>, KnowledgeError> {
        let mut all: Vec<KnowledgeItem> = {
            let guard = self
                .items
                .lock()
                .map_err(|_| KnowledgeError::StoreUnavailable)?;
            guard.values().cloned().collect()
        };
        // Deterministic order: evidence_count DESC, then id ASC as tiebreak.
        all.sort_by(|a, b| {
            b.evidence_count
                .cmp(&a.evidence_count)
                .then(a.id.cmp(&b.id))
        });
        let q = query.to_lowercase();
        let scoped = filter_by_scope(&all, scope);
        let result = scoped
            .into_iter()
            .filter(|i| q.is_empty() || i.text_ref.to_lowercase().contains(&q))
            .take(limit as usize)
            .cloned()
            .collect();
        Ok(result)
    }

    fn put_candidate(&self, item: KnowledgeItem) -> Result<(), KnowledgeError> {
        promote_candidate(&item)?;
        let mut guard = self
            .items
            .lock()
            .map_err(|_| KnowledgeError::StoreUnavailable)?;
        guard.insert(item.id.clone(), item);
        Ok(())
    }
}
