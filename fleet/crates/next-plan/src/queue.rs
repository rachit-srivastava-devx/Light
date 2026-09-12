use crate::types::{NextError, NextPlanSignal};
use std::collections::HashSet;

pub trait NextQueue {
    fn push(&mut self, signal: NextPlanSignal) -> Result<bool, NextError>;
    fn depth(&self) -> usize;
    fn next_cursor(&mut self) -> u64;
}

pub struct MemQueue {
    capacity: usize,
    entries: Vec<NextPlanSignal>,
    seen: HashSet<String>,
    cursor: u64,
}

impl MemQueue {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity >= 1, "MemQueue capacity must be at least 1");
        Self {
            capacity,
            entries: Vec::new(),
            seen: HashSet::new(),
            cursor: 0,
        }
    }
}

impl NextQueue for MemQueue {
    fn push(&mut self, signal: NextPlanSignal) -> Result<bool, NextError> {
        if self.seen.contains(&signal.parent_digest) {
            return Ok(false);
        }
        if self.entries.len() >= self.capacity {
            return Err(NextError::Backpressure);
        }
        self.seen.insert(signal.parent_digest.clone());
        self.entries.push(signal);
        Ok(true)
    }

    fn depth(&self) -> usize {
        self.entries.len()
    }

    fn next_cursor(&mut self) -> u64 {
        self.cursor += 1;
        self.cursor
    }
}

/// Emit a `NextPlanSignal` to the queue.
///
/// A signal with an empty `parent_digest` (no prior plan) is a no-op:
/// it returns `Ok(())` without touching the queue.
/// Duplicate signals (same `parent_digest`) are silently dropped.
pub fn emit_signal(signal: NextPlanSignal, queue: &mut dyn NextQueue) -> Result<(), NextError> {
    if signal.parent_digest.is_empty() {
        return Ok(());
    }
    queue.push(signal)?;
    Ok(())
}
