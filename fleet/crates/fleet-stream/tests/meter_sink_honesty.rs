//! `MeterSink` never fabricates a dollar figure from a token count and a guessed rate.

#[path = "log_source_fake.rs"]
mod fake;

use fleet_types::ReceiptEvent;
use fleet_stream::sinks::MeterSink;
use fleet_stream::{Sink, StreamEvent};
use serde_json::json;

use fake::make_receipt;

#[test]
fn meter_sink_never_fabricates_a_dollar_figure() {
    let (mut sink, snapshot) = MeterSink::new(Some(1000));
    let receipt = make_receipt(1, ReceiptEvent::RunEnd, json!({"tokens_in": 40, "tokens_out": 10}));
    let event = StreamEvent(receipt);
    assert!(sink.accepts(&event));
    sink.deliver(&event).unwrap();
    let guard = snapshot.lock().unwrap();
    let sample = guard.samples[0];
    assert_eq!(sample.estimated_tokens, 50);
    assert_eq!(sample.labelled_cost_cents, None);
    assert_eq!(sample.window_pct, Some(5));
}
