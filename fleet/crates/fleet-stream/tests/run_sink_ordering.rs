//! Ordering/backlog/cursor-advance behavior for `run_sink`: bounded-but-never-drops.

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

fn fast_config(queue_capacity: usize) -> PumpConfig {
    PumpConfig {
        queue_capacity,
        poll_interval: Duration::from_millis(5),
        max_retries: 3,
        retry_backoff: RetryBackoff { base: Duration::from_millis(1), max: Duration::from_millis(5) },
    }
}

#[test]
fn stream_event_seq_matches_receipt_seq() {
    for (seq, event) in [(1, ReceiptEvent::RunStart), (7, ReceiptEvent::LaneStatus)] {
        let receipt = make_receipt(seq, event, json!({}));
        let expected = receipt.seq;
        assert_eq!(fleet_stream::StreamEvent(receipt).seq(), expected);
    }
}

#[tokio::test]
async fn pump_config_queue_capacity_bounds_but_never_drops() {
    let receipts = (1..=500).map(|seq| make_receipt(seq, ReceiptEvent::RunStart, json!({}))).collect();
    let source = FakeLogSource::new(receipts);
    let cursors = FakeCursorStore::new();
    let mut sink = FakeSink::new("bounded");
    let delivered = sink.delivered_handle();
    let config = fast_config(50);
    let (tx, rx) = watch::channel(false);
    let mut shutdown = rx.clone();
    let handle = tokio::spawn(async move {
        run_sink(&source, &cursors, &mut sink, &config, &mut shutdown).await
    });
    tokio::time::sleep(Duration::from_millis(300)).await;
    let _ = tx.send(true);
    let stats = handle.await.unwrap().unwrap();
    assert_eq!(stats.delivered, 500);
    assert_eq!(delivered.lock().unwrap().len(), 500);
}
