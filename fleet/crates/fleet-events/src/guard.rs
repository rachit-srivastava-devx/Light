//! `guard` -- the one mandatory checkpoint before a side-effecting `EventKind` becomes an action.

use crate::envelope::EventEnvelope;
use crate::event_kind::EventKind;
use crate::ids::EventId;

/// Quote is capped to this many bytes (of the JSON-serialized `payload`).
pub const MAX_QUOTE_BYTES: usize = 2_048;

const TRUNCATION_MARKER: &str = "…[truncated]";

/// What a caller must look at (and, if `requires_confirmation`, get explicit approval on) before
/// turning `kind` into an action.
#[derive(Clone, Debug, serde::Serialize)]
pub struct GuardedAction {
    pub envelope_id: EventId,
    pub kind: EventKind,
    /// Verbatim, byte-capped excerpt of `payload` -- never summarized or translated.
    pub quote: String,
    pub requires_confirmation: bool,
}

/// Pure and total: no IO, never panics. `envelope.kind.side_effecting()` alone decides
/// `requires_confirmation` -- this fn never inspects `payload` to make that decision, only to
/// build the quote a human/kernel reads.
pub fn guard(envelope: &EventEnvelope) -> GuardedAction {
    let serialized = serde_json::to_string(&envelope.payload).unwrap_or_default();
    let quote = cap_quote(&serialized);
    GuardedAction {
        envelope_id: envelope.id.clone(),
        kind: envelope.kind,
        quote,
        requires_confirmation: envelope.kind.side_effecting(),
    }
}

fn cap_quote(serialized: &str) -> String {
    if serialized.len() <= MAX_QUOTE_BYTES {
        return serialized.to_string();
    }
    let mut end = MAX_QUOTE_BYTES;
    while end > 0 && !serialized.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{TRUNCATION_MARKER}", &serialized[..end])
}
