//! `ingest_once()` with a fake `Adapter` + fake `EventSink` -- BLUEPRINT.md §9 (happy paths).

mod support;

use fleet_events::{ingest_once, EventEnvelope, EventId, EventKind, SourceKind};
use serde_json::json;
use std::cell::RefCell;
use support::{FakeAdapter, FakeSink, FixedClock};

fn envelope(kind: EventKind, ext: &str) -> EventEnvelope {
    let id = EventId::derive(SourceKind::Cli, kind, ext);
    EventEnvelope::new(id, SourceKind::Cli, kind, "2026-01-01T00:00:00Z".to_string(), json!({}))
}

#[test]
fn ingest_once_empty_pull_is_a_clean_success() {
    let mut adapter = FakeAdapter { batch: vec![] };
    let sink = FakeSink { written: RefCell::new(vec![]), fail_at: None };
    let report = ingest_once(&mut adapter, &sink, &FixedClock).unwrap();
    assert_eq!(report.pulled, 0);
    assert_eq!(report.written, 0);
    assert!(report.guarded.is_empty());
}

#[test]
fn ingest_once_guards_every_pulled_envelope() {
    let batch = vec![
        envelope(EventKind::CliInvoked, "a"),
        envelope(EventKind::GithubPush, "b"),
        envelope(EventKind::FsFileCreated, "c"),
    ];
    let mut adapter = FakeAdapter { batch: batch.clone() };
    let sink = FakeSink { written: RefCell::new(vec![]), fail_at: None };
    let report = ingest_once(&mut adapter, &sink, &FixedClock).unwrap();
    assert_eq!(report.guarded.len(), batch.len());
    assert_eq!(report.pulled, batch.len());
}

#[test]
fn duplicate_id_within_a_batch_is_a_sink_no_op_not_a_crash() {
    let dup = envelope(EventKind::CliInvoked, "same");
    let batch = vec![dup.clone(), dup];
    let mut adapter = FakeAdapter { batch };
    let sink = FakeSink { written: RefCell::new(vec![]), fail_at: None };
    let report = ingest_once(&mut adapter, &sink, &FixedClock).unwrap();
    assert_eq!(report.written, 2);
    assert_eq!(sink.written.borrow().len(), 1);
}
