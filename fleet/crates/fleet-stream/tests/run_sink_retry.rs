//! `Transient` retry-then-`Permanent`-downgrade behavior.

#[path = "log_source_fake.rs"]
mod fake;
#[path = "sink_fake.rs"]
mod sink_fake;

use std::time::Duration;

use fleet_types::ReceiptEvent;
use fleet_stream::{run_sink, PumpConfig, RetryBackoff};
use serde_json::json;
use tokio::sync::watch;

use fake::{make_receipt, FakeCursorStore, FakeLogSource};
use sink_fake::FakeSink;

fn fast_config(max_retries: u32) -> PumpConfig {
    PumpConfig {
        queue_capacity: 100,
        poll_interval: Duration::from_millis(5),
        max_retries,
        retry_backoff: RetryBackoff { base: Duration::from_millis(1), max: Duration::from_millis(5) },
    }
}

async fn drive(
    source: FakeLogSource,
    cursors: FakeCursorStore,
    mut sink: FakeSink,
    config: PumpConfig,
    settle: Duration,
) -> fleet_stream::SinkStats {
    let (tx, mut rx) = watch::channel(false);
    let handle = tokio::spawn(async move { run_sink(&source, &cursors, &mut sink, &config, &mut rx).await });
    tokio::time::sleep(settle).await;
    let _ = tx.send(true);
    handle.await.unwrap().unwrap()
}

#[tokio::test]
async fn run_sink_transient_failure_retries_then_advances() {
    let receipts = vec![make_receipt(1, ReceiptEvent::RunStart, json!({}))];
    let sink = FakeSink::new("flaky").with_transient(1, 2);
    let stats = drive(
        FakeLogSource::new(receipts),
        FakeCursorStore::new(),
        sink,
        fast_config(5),
        Duration::from_millis(100),
    )
    .await;
    assert!(stats.retried >= 2, "expected >=2 retries, got {}", stats.retried);
    assert_eq!(stats.delivered, 1);
    assert_eq!(stats.last_delivered_seq, Some(1));
}

#[tokio::test]
async fn run_sink_permanent_failure_does_not_stall_later_events() {
    let receipts: Vec<_> = (1..=10).map(|seq| make_receipt(seq, ReceiptEvent::RunStart, json!({}))).collect();
    let sink = FakeSink::new("poison").with_permanent(5);
    let stats = drive(
        FakeLogSource::new(receipts),
        FakeCursorStore::new(),
        sink,
        fast_config(3),
        Duration::from_millis(150),
    )
    .await;
    assert_eq!(stats.permanently_skipped, 1);
    assert_eq!(stats.delivered, 9);
    assert_eq!(stats.last_delivered_seq, Some(10));
}
