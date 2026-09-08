//! `Timestamp`, `Importance`, and `MemoryItem` — the caller-supplied clock and stored row shape.
//! See BLUEPRINT.md §3.C.

use crate::embedding::Embedding;
use crate::ident::{MemoryId, MemoryKind};

/// Caller-supplied wall-clock reading, seconds since Unix epoch. This crate never calls
/// `SystemTime::now()`.
/// Also derives `Serialize`/`Deserialize` beyond §3's snippet, for the same `MemoryItem`
/// compile-requirement reason as `MemoryId` (see `ident.rs`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct Timestamp(u64);
impl Timestamp {
    pub fn from_unix_secs(secs: u64) -> Self {
        Timestamp(secs)
    }
    pub fn unix_secs(self) -> u64 {
        self.0
    }
}

/// A `[0.0, 1.0]`-clamped importance rating.
/// Also derives `Serialize`/`Deserialize` beyond §3's snippet, for the same `MemoryItem`
/// compile-requirement reason as `MemoryId` (see `ident.rs`).
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Importance(f64);
/// `Importance::new` was given a value outside `[0.0, 1.0]` or non-finite (`NaN`/`inf`).
#[derive(Clone, Copy, Debug, PartialEq, thiserror::Error)]
#[error("importance {0} is not finite and within [0.0, 1.0]")]
pub struct ImportanceOutOfRange(pub f64);
impl Importance {
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

/// One durable memory row — the shape `write`/`score`/`promote_lesson` all operate over.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct MemoryItem {
    pub id: MemoryId,
    pub kind: MemoryKind,
    pub text: String,
    pub embedding: Embedding,
    pub importance: Importance,
    pub created_at: Timestamp,
    pub last_confirmed_at: Timestamp,
    pub confirmed_count: u32,
}
