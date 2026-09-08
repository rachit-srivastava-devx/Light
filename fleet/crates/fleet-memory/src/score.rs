//! `score` — `alpha*recency + beta*importance + gamma*relevance`. See BLUEPRINT.md §3.D.

use crate::item::{ImportanceOutOfRange, MemoryItem, Timestamp};

/// A retrieval-time relevance signal for one item against one query, produced by `retrieve`'s
/// fusion (§E) — never computed by `score` itself.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Relevance(f64);
impl Relevance {
    pub fn new(value: f64) -> Result<Self, ImportanceOutOfRange> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(ImportanceOutOfRange(value));
        }
        Ok(Self(value))
    }
    pub fn get(self) -> f64 {
        self.0
    }
}

/// The three fusion weights, caller-supplied (a scoring-policy choice, not hardcoded here).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScoreWeights {
    pub alpha: f64,
    pub beta: f64,
    pub gamma: f64,
}

/// The fused score. Not clamped to any range.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Score(f64);
impl Score {
    pub fn get(self) -> f64 {
        self.0
    }
}

/// Recency half-life, in seconds.
pub const RECENCY_HALF_LIFE_SECS: u64 = 14 * 24 * 3600;

/// Pure, total, never panics.
pub fn score(item: &MemoryItem, now: Timestamp, relevance: Relevance, weights: ScoreWeights) -> Score {
    let age_secs = now.unix_secs().saturating_sub(item.last_confirmed_at.unix_secs());
    let recency = 0.5_f64.powf(age_secs as f64 / RECENCY_HALF_LIFE_SECS as f64);
    Score(weights.alpha * recency + weights.beta * item.importance.get() + weights.gamma * relevance.get())
}
