use ingest::IncomingEvent;
use ingest::{normalize, InboxDecision, SeenIds};
use serde_json::json;

#[path = "../common/mod.rs"]
mod common;
use common::reg;

#[test]
fn same_delivery_is_idempotent() {
    let mut seen = SeenIds::new();
    assert!(matches!(
        seen.record("github", "d1", "digest-a"),
        InboxDecision::Accepted { .. }
    ));
    assert!(matches!(
        seen.record("github", "d1", "digest-a"),
        InboxDecision::Duplicate { .. }
    ));
}

#[test]
fn digest_conflict_refuses() {
    let mut seen = SeenIds::new();
    seen.record("github", "d1", "digest-a");
    assert!(matches!(
        seen.record("github", "d1", "digest-b"),
        InboxDecision::Conflict { .. }
    ));
}

#[test]
fn seenids_with_capacity_zero_is_safe() {
    // Must not panic — zero capacity is clamped to 1.
    let mut seen = SeenIds::with_capacity(0);
    assert!(matches!(
        seen.record("s", "1", "d1"),
        InboxDecision::Accepted { .. }
    ));
}

#[test]
fn seenids_evicts_at_capacity() {
    let mut seen = SeenIds::with_capacity(2);
    seen.record("s", "1", "d1");
    seen.record("s", "2", "d2");
    seen.record("s", "3", "d3"); // evicts "s\x001"
                                 // Re-delivering event 1 must be accepted again (LRU evicted it; at-least-once semantics).
    assert!(
        matches!(seen.record("s", "1", "d1"), InboxDecision::Accepted { .. }),
        "evicted key must be accepted again"
    );
}

#[test]
fn store_row_contains_no_raw_secret() {
    let ev = normalize(
        IncomingEvent {
            source: "src".into(),
            delivery_id: "d1".into(),
            payload: json!({"password": "hunter2"}),
            attachments: vec![],
        },
        &reg(),
    )
    .unwrap();
    let stored = serde_json::to_string(&ev.payload).unwrap();
    assert!(
        !stored.contains("hunter2"),
        "raw secret must not appear in stored payload"
    );
    assert_eq!(ev.redaction_receipt.field_count, 1);
}
