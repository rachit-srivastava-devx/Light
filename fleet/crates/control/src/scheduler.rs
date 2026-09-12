use crate::{ControlError, Snapshot};
use std::collections::BinaryHeap;

/// Scored entry: higher score = higher priority; tie-break ascending task_id.
#[derive(Eq, PartialEq)]
struct Entry {
    score: u64,
    task_id: String,
}

impl Ord for Entry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Higher score → popped first (max-heap on score).
        // Equal score → smaller task_id wins (deterministic, reproducible).
        self.score
            .cmp(&other.score)
            .then_with(|| other.task_id.cmp(&self.task_id))
    }
}

impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Deterministic integer-scored ready heap.
/// Same inputs always produce the same pop order (reproducible from a snapshot).
pub struct ReadyHeap {
    inner: BinaryHeap<Entry>,
}

impl ReadyHeap {
    pub fn new() -> Self {
        Self { inner: BinaryHeap::new() }
    }

    pub fn push(&mut self, task_id: String, score: u64) {
        self.inner.push(Entry { score, task_id });
    }

    pub fn pop(&mut self) -> Option<String> {
        self.inner.pop().map(|e| e.task_id)
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl Default for ReadyHeap {
    fn default() -> Self {
        Self::new()
    }
}

/// CAS guard: errors when the snapshot revision differs from the expected
/// value, preventing split-brain reducer decisions.
pub fn cas_guard(snapshot: &Snapshot, expected: u64) -> Result<(), ControlError> {
    if snapshot.revision != expected {
        Err(ControlError::Store(format!(
            "CAS conflict: expected revision {expected} got {}",
            snapshot.revision
        )))
    } else {
        Ok(())
    }
}
