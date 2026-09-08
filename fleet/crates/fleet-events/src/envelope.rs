//! `EventEnvelope` -- the one typed shape every ingress source normalizes into.

use crate::event_kind::EventKind;
use crate::ids::EventId;
use crate::source_kind::SourceKind;
use serde_json::Value;

/// `payload` is truncated to this many bytes (applies to the JSON-serialized form) before an
/// `EventEnvelope` is constructed.
pub const MAX_PAYLOAD_BYTES: usize = 262_144; // 256 KiB

/// One normalized ingress event. `payload` is untrusted, never parsed for control flow; `kind` is
/// the only dispatch key.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EventEnvelope {
    pub id: EventId,
    pub source: SourceKind,
    pub kind: EventKind,
    /// RFC3339, stamped by the adapter from its injected `Clock`.
    pub received_at: String,
    pub payload: Value,
}

impl EventEnvelope {
    /// Constructs an envelope, capping `payload`'s serialized size to `MAX_PAYLOAD_BYTES`. Never
    /// errors: an oversized payload is truncated to a JSON string marker, never rejected.
    pub fn new(
        id: EventId,
        source: SourceKind,
        kind: EventKind,
        received_at: String,
        payload: Value,
    ) -> Self {
        let payload = cap_payload(payload);
        Self { id, source, kind, received_at, payload }
    }
}

fn cap_payload(payload: Value) -> Value {
    let serialized = serde_json::to_string(&payload).unwrap_or_default();
    if serialized.len() <= MAX_PAYLOAD_BYTES {
        return payload;
    }
    let mut end = MAX_PAYLOAD_BYTES;
    while end > 0 && !serialized.is_char_boundary(end) {
        end -= 1;
    }
    Value::String(format!("{}…[truncated]", &serialized[..end]))
}
