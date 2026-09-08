//! A slow sink never backpressures the pipeline: independent per-sink progress.

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
async fn two_sinks_progress_independently() {
    let receipts: Vec<_> = (1..=5).map(|seq| make_receipt(seq, ReceiptEvent::RunStart, json!({}))).collect();
    // Bounded (not literally infinite, so a stray bug cannot hang the suite), but at 200
    // retries with backoff capped at 5ms this sink is still deep in retries long after the
    // "normal" sink -- unblocked by it -- has already finished delivering everything.
    let blocked = FakeSink::new("blocked").with_transient(1, 200);
    let normal = FakeSink::new("normal");
    let normal_delivered = normal.delivered_handle();

    let blocked_handle = tokio::spawn(drive(
        FakeLogSource::new(receipts.clone()),
        FakeCursorStore::new(),
        blocked,
        fast_config(200),
        Duration::from_secs(2),
    ));
    tokio::time::sleep(Duration::from_millis(5)).await;
    let normal_stats = drive(
        FakeLogSource::new(receipts),
        FakeCursorStore::new(),
        normal,
        fast_config(3),
        Duration::from_millis(30),
    )
    .await;

    assert_eq!(normal_stats.delivered, 5);
    assert_eq!(normal_delivered.lock().unwrap().len(), 5);
    // "normal" already finished while "blocked" is still mid-retry -- proof one sink's
    // slowness never delays another's progress.
    assert!(!blocked_handle.is_finished(), "blocked sink should still be retrying");
    blocked_handle.abort();
}
