//! `EventId` -- deterministic identity derived from `(source, kind, external_id)`.

use crate::event_kind::EventKind;
use crate::source_kind::SourceKind;
use fleet_types::Blake3Hash;

/// Stable identity of one ingested event. See the crate-level docs (`lib.rs`) for why this must
/// be a deterministic hash, never a random or time-based value.
#[derive(Clone, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct EventId(Blake3Hash);

// `Blake3Hash` has no `Ord`/`PartialOrd` impl (fleet-types owns that type), so `EventId`'s total
// order is defined here, over its wire string form -- deterministic and stable since `Blake3Hash`
// is itself immutable once constructed.
impl Ord for EventId {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.as_str().cmp(other.0.as_str())
    }
}

impl PartialOrd for EventId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl EventId {
    /// Hashes `(source, kind, external_id)` together so the same `external_id` string reused by
    /// two different sources/kinds can never collide (see `distinct_sources_never_collide_on_shared_external_id`).
    pub fn derive(source: SourceKind, kind: EventKind, external_id: &str) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(source.wire_tag().as_bytes());
        hasher.update(b"\0");
        hasher.update(kind.wire_tag().as_bytes());
        hasher.update(b"\0");
        hasher.update(external_id.as_bytes());
        let digest = hasher.finalize().to_hex();
        let formatted = format!("blake3:{digest}");
        Self(Blake3Hash::parse(formatted).expect("blake3 hex digest is always well-formed"))
    }

    pub fn as_blake3(&self) -> &Blake3Hash {
        &self.0
    }
}
