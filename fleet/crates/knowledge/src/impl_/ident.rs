//! `MemoryId` and `MemoryKind` — the episodic/semantic/procedural split absent from today's flat
//! `memories` table (`memory_store.py:52-64`). See BLUEPRINT.md §3.A.

/// Non-empty stable identifier for one memory row.
///
/// Derives `Serialize`/`Deserialize` beyond BLUEPRINT.md §3's literal snippet: `MemoryItem`
/// (§3.C) derives them too and embeds a `MemoryId`, so this is required for that struct to
/// compile — a minimal, necessary deviation (see the crate's return notes).
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct MemoryId(String);

/// A `MemoryId` was constructed from an empty or all-whitespace string.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("memory id must not be empty")]
pub struct EmptyMemoryId;

impl MemoryId {
    pub fn parse(value: impl Into<String>) -> Result<Self, EmptyMemoryId> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(EmptyMemoryId);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The three-way split: `Episodic` (a dated event), `Semantic` (a durable fact/correction, today's
/// only kind), `Procedural` (a reusable how-to). Carried through scoring/dedup/promotion, never
/// interpreted differently per kind by this crate.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    Episodic,
    Semantic,
    Procedural,
}
