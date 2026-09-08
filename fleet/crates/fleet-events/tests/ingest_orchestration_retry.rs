//! `ingest_once()` sink-failure/retry contract -- BLUEPRINT.md §9 (split from
//! `ingest_orchestration.rs` to stay under the 80-line-per-file gate).

mod support;

use fleet_events::{ingest_once, EventEnvelope, EventId, EventKind, IngestError, SourceKind};
use serde_json::json;
use std::cell::RefCell;
use support::{FakeAdapter, FakeSink, FixedClock};

fn envelope(kind: EventKind, ext: &str) -> EventEnvelope {
    let id = EventId::derive(SourceKind::Cli, kind, ext);
    EventEnvelope::new(id, SourceKind::Cli, kind, "2026-01-01T00:00:00Z".to_string(), json!({}))
}

#[test]
fn ingest_once_stops_at_first_sink_failure_and_is_safely_retryable() {
    let batch = vec![
        envelope(EventKind::CliInvoked, "a"),
        envelope(EventKind::CliInvoked, "b"),
        envelope(EventKind::CliInvoked, "c"),
    ];
    let written = RefCell::new(vec![]);
    {
        let mut adapter = FakeAdapter { batch: batch.clone() };
        let sink = FakeSink { written: RefCell::new(vec![]), fail_at: Some(1) };
        let err = ingest_once(&mut adapter, &sink, &FixedClock).unwrap_err();
        assert!(matches!(err, IngestError::Sink(_)));
        assert_eq!(sink.written.borrow().len(), 1);
        written.replace(sink.written.into_inner());
    }
    let mut adapter = FakeAdapter { batch };
    let sink = FakeSink { written: RefCell::new(written.into_inner()), fail_at: None };
    let report = ingest_once(&mut adapter, &sink, &FixedClock).unwrap();
    assert_eq!(report.written, 3);
}
