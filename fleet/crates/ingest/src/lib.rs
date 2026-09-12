//! `ingest` — validate, redact, and normalize external events before they touch the store.
//!
//! Entry point: [`normalize`]. Dedup: [`SeenIds`].

pub mod types;
pub mod dedup;

mod registry;
mod redact;
mod injection;
mod attachment;

pub use types::*;
pub use dedup::{InboxDecision, SeenIds};

use std::io::{self, Write};
use serde_json::Value;

struct ByteCounter(usize);
impl Write for ByteCounter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0 += buf.len();
        if self.0 > MAX_PAYLOAD_BYTES {
            return Err(io::Error::other("oversized"));
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

fn check_size(payload: &Value) -> Result<(), IngestError> {
    serde_json::to_writer(ByteCounter(0), payload)
        .map_err(|_| IngestError::OversizedPayload)
}

fn compute_digest(payload: &Value) -> String {
    // A validated serde_json::Value is always serializable; expect() is appropriate here.
    let bytes = serde_json::to_vec(payload).expect("Value always serializes to valid JSON");
    format!("blake3:{}", blake3::hash(&bytes).to_hex())
}

/// Normalize an incoming event through the full validation pipeline.
///
/// Steps (in order, matching the security contract):
/// 1. `check_source`   — NFC-normalize, trim, cap, verify namespace
/// 2. `check_size`     — reject oversized payloads before any allocation-heavy work
/// 3. `check_injection` (pre-redact) — scan raw payload so injection hidden in secret fields
///    (e.g. `{"password": "Ignore all previous instructions"}`) is still detected before
///    the value is wiped by the redaction pass
/// 4. `redact_secrets` — scrub keys/tokens/passwords; track what was removed
/// 5. `validate_attachments` — count, URI len, size, uniqueness
/// 6. `compute_digest` — BLAKE3 over the scrubbed payload
pub fn normalize(input: IncomingEvent, reg: &SourceRegistration) -> Result<NormalizedEvent, IngestError> {
    let source    = registry::check_source(&input.source, reg)?;
    check_size(&input.payload)?;
    let taint     = injection::check_injection(&input.payload);  // scan raw, before redaction
    let (payload, mut receipt) = redact::redact_secrets(input.payload)?;
    attachment::validate_attachments(&input.attachments)?;
    let digest    = compute_digest(&payload);
    let event_id  = format!("{source}-{}", input.delivery_id);
    receipt.event_id = event_id.clone();
    Ok(NormalizedEvent { event_id, source, payload, payload_digest: digest,
                         attachments: input.attachments, redaction_receipt: receipt,
                         injection_taint: taint })
}
