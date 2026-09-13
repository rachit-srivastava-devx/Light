use crate::types::MAX_SEEN_IDS;
use lru::LruCache;
use std::num::NonZeroUsize;

#[derive(Debug, PartialEq)]
pub enum InboxDecision {
    Accepted {
        event_id: String,
    },
    Duplicate {
        event_id: String,
    },
    Conflict {
        prior_digest: String,
        new_digest: String,
    },
}

/// LRU-bounded dedup tracker. Key: `(source, delivery_id)`. Value: `payload_digest`.
/// Evicts oldest entries at `MAX_SEEN_IDS` to prevent unbounded memory growth.
pub struct SeenIds(LruCache<String, String>);

impl SeenIds {
    pub fn new() -> Self {
        Self::with_capacity(MAX_SEEN_IDS)
    }

    pub fn with_capacity(cap: usize) -> Self {
        let n = NonZeroUsize::new(cap.max(1)).expect("max(1) is always nonzero");
        Self(LruCache::new(n))
    }

    pub fn record(
        &mut self,
        source: &str,
        delivery_id: &str,
        payload_digest: &str,
    ) -> InboxDecision {
        let key = format!("{source}\x00{delivery_id}");
        let event_id = format!("{source}-{delivery_id}");
        match self.0.get(&key) {
            Some(prior) if prior == payload_digest => InboxDecision::Duplicate { event_id },
            Some(prior) => InboxDecision::Conflict {
                prior_digest: prior.clone(),
                new_digest: payload_digest.to_string(),
            },
            None => {
                self.0.put(key, payload_digest.to_string());
                InboxDecision::Accepted { event_id }
            }
        }
    }
}

impl Default for SeenIds {
    fn default() -> Self {
        Self::new()
    }
}
