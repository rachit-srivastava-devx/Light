//! `MeterSink`: honest cost/usage aggregation. Reports only what it can measure -- estimated
//! tokens and window percentage -- and a labelled `$` figure only when the receipt body itself
//! carried one. Never derives a price from a token count and a guessed rate.

use std::sync::{Arc, Mutex};

use fleet_types::ReceiptEvent;
use serde_json::Value;

use super::meter_types::{MeterSample, MeterSnapshot};
use crate::event::StreamEvent;
use crate::sink::{Sink, SinkError};

pub struct MeterSink {
    window_capacity: Option<u64>,
    snapshot: Arc<Mutex<MeterSnapshot>>,
}

impl MeterSink {
    /// `window_capacity` is the token budget `window_pct` is measured against; `None` (rather
    /// than a guess) leaves `window_pct` unset on every sample.
    pub fn new(window_capacity: Option<u64>) -> (Self, Arc<Mutex<MeterSnapshot>>) {
        let snapshot = Arc::new(Mutex::new(MeterSnapshot::default()));
        (
            Self { window_capacity, snapshot: Arc::clone(&snapshot) },
            snapshot,
        )
    }
}

fn body_u64(body: Option<&serde_json::Map<String, Value>>, key: &str) -> Option<u64> {
    body?.get(key).and_then(Value::as_u64)
}

impl Sink for MeterSink {
    fn id(&self) -> &'static str {
        "meter"
    }

    fn accepts(&self, event: &StreamEvent) -> bool {
        if event.0.event == ReceiptEvent::GateVerdict {
            return true;
        }
        let body = event.0.body.as_object();
        ["cost_cents", "cost", "tokens_in", "tokens_out"]
            .iter()
            .any(|key| body.is_some_and(|b| b.contains_key(*key)))
    }

    fn deliver(&mut self, event: &StreamEvent) -> Result<(), SinkError> {
        let body = event.0.body.as_object();
        let estimated_tokens =
            body_u64(body, "tokens_in").unwrap_or(0) + body_u64(body, "tokens_out").unwrap_or(0);
        let labelled_cost_cents = body_u64(body, "cost_cents").or_else(|| body_u64(body, "cost"));
        let window_pct = self
            .window_capacity
            .filter(|cap| *cap > 0)
            .map(|cap| ((estimated_tokens.min(cap) * 100) / cap) as u8);
        let sample = MeterSample { seq: event.seq(), estimated_tokens, window_pct, labelled_cost_cents };
        let mut guard = self.snapshot.lock().map_err(|_| SinkError::Permanent {
            sink: "meter",
            seq: event.seq(),
            reason: "meter mutex poisoned".into(),
        })?;
        guard.samples.push(sample);
        Ok(())
    }
}
