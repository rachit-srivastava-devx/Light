//! `accepts()` per sink type, table-driven, plus the `SinkError` closed-variant-set check.

#[path = "log_source_fake.rs"]
mod fake;

use fleet_types::ReceiptEvent;
use fleet_stream::sinks::{DashboardSink, FileSink, OrbSink, WebhookConfig, WebhookSink};
use fleet_stream::{Sink, SinkError, StreamEvent};
use serde_json::json;

use fake::make_receipt;

const ALL_EVENTS: [ReceiptEvent; 8] = [
    ReceiptEvent::RunStart,
    ReceiptEvent::ArtifactFrozen,
    ReceiptEvent::Attested,
    ReceiptEvent::Refusal,
    ReceiptEvent::GateVerdict,
    ReceiptEvent::RunEnd,
    ReceiptEvent::LaneStatus,
    ReceiptEvent::Rollback,
];

#[test]
fn orb_sink_accepts_only_lane_status() {
    let sink = OrbSink::new("http://127.0.0.1:1/lane-status");
    for event in ALL_EVENTS {
        let receipt = make_receipt(1, event, json!({"lane_id": "lead"}));
        let accepted = sink.accepts(&StreamEvent(receipt));
        assert_eq!(accepted, event == ReceiptEvent::LaneStatus, "event {event:?}");
    }
}

#[test]
fn webhook_sink_default_allow_list_accepts_everything() {
    let sink = WebhookSink::new(WebhookConfig {
        url: "http://127.0.0.1:1/hook".into(),
        allowed: Vec::new(),
        bearer_token: None,
    });
    for event in ALL_EVENTS {
        let receipt = make_receipt(1, event, json!({}));
        assert!(sink.accepts(&StreamEvent(receipt)));
    }
}

#[test]
fn dashboard_sink_accepts_everything() {
    let sink = DashboardSink::new();
    for event in ALL_EVENTS {
        let receipt = make_receipt(1, event, json!({}));
        assert!(sink.accepts(&StreamEvent(receipt)));
    }
}

#[test]
fn file_sink_default_allow_list_accepts_everything() {
    let sink = FileSink::new("/tmp/does-not-need-to-exist.ndjson", Vec::new());
    for event in ALL_EVENTS {
        let receipt = make_receipt(1, event, json!({}));
        assert!(sink.accepts(&StreamEvent(receipt)));
    }
}

#[test]
fn sink_error_transient_vs_permanent_are_the_only_variants() {
    let err = SinkError::Transient { sink: "x", reason: "r".into() };
    match err {
        SinkError::Transient { .. } => {}
        SinkError::Permanent { .. } => {}
    }
}
