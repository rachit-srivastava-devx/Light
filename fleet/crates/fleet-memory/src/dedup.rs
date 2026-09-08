//! `write` — dedup-on-write: merge `cos >= tau`, never blind-append. See BLUEPRINT.md §3.F.

use crate::embedding::{CosineSimilarity, Embedding};
use crate::ident::{MemoryId, MemoryKind};
use crate::item::{Importance, ImportanceOutOfRange, MemoryItem, Timestamp};
use crate::retrieve::RetrieveError;

/// A not-yet-persisted candidate memory.
#[derive(Clone, Debug)]
pub struct NewMemory {
    pub kind: MemoryKind,
    pub text: String,
    pub embedding: Embedding,
    pub importance: Importance,
}

/// The injected nearest-neighbor port `write` consults before deciding.
pub trait NearestNeighborLookup {
    fn nearest(&self, embedding: &Embedding) -> Result<Option<(MemoryId, CosineSimilarity)>, RetrieveError>;
}

/// The dedup threshold tau: a `nearest` hit with similarity `>= tau` is a duplicate to merge.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct DedupThreshold(f64);
impl DedupThreshold {
    /// Rejects only non-finite values (`NaN`/`inf`) — a negative or out-of-`[-1,1]` `tau` is a
    /// legal, if extreme, threshold choice (see BLUEPRINT.md §6's "negative" row for `write`).
    pub fn new(value: f64) -> Result<Self, ImportanceOutOfRange> {
        if !value.is_finite() {
            return Err(ImportanceOutOfRange(value));
        }
        Ok(Self(value))
    }
}

/// The pure outcome of a `write` call — persistence itself is the caller's job.
#[derive(Clone, Debug)]
pub enum WriteDecision {
    /// No existing item is within `tau` — insert `candidate` as a brand-new row.
    Insert(MemoryItem),
    /// An existing item at `into` is within `tau` — merge instead of appending.
    Merge { into: MemoryId, similarity: CosineSimilarity, new_importance: Importance, new_confirmed_count: u32 },
}

/// Pure and total. Looks up `existing.nearest(&candidate.embedding)`; `sim >= tau` merges,
/// otherwise inserts. `write` sees only `(id, similarity)` from the port (no full existing item),
/// so the merge fields carry the candidate's own observation forward — the caller applies them
/// against the real stored item's `importance`/`confirmed_count` (documented deviation, see the
/// crate's return notes: §3's `Merge` doc text describes a "max of the two"/increment that needs
/// the existing item's values, which `NearestNeighborLookup` does not expose).
pub fn write(
    id: MemoryId,
    candidate: NewMemory,
    now: Timestamp,
    tau: DedupThreshold,
    existing: &dyn NearestNeighborLookup,
) -> Result<WriteDecision, RetrieveError> {
    if let Some((into, similarity)) = existing.nearest(&candidate.embedding)? {
        if similarity.get() >= tau.0 {
            return Ok(WriteDecision::Merge {
                into,
                similarity,
                new_importance: candidate.importance,
                new_confirmed_count: 1,
            });
        }
    }
    Ok(WriteDecision::Insert(MemoryItem {
        id,
        kind: candidate.kind,
        text: candidate.text,
        embedding: candidate.embedding,
        importance: candidate.importance,
        created_at: now,
        last_confirmed_at: now,
        confirmed_count: 0,
    }))
}
