//! At-least-once resume: a crashed sink restarts from its persisted cursor, never replaying.

#[path = "log_source_fake.rs"]
mod fake;
#[path = "sink_fake.rs"]
mod sink_fake;

use std::sync::Arc;
use std::time::Duration;

use fleet_types::ReceiptEvent;
use fleet_stream::{run_sink, PumpConfig, RetryBackoff};
use serde_json::json;
use tokio::sync::watch;

use fake::{make_receipt, FakeCursorStore, FakeLogSource};
use sink_fake::FakeSink;

fn fast_config() -> PumpConfig {
    PumpConfig {
        queue_capacity: 100,
        poll_interval: Duration::from_millis(5),
        max_retries: 3,
        retry_backoff: RetryBackoff { base: Duration::from_millis(1), max: Duration::from_millis(5) },
    }
}

#[tokio::test]
async fn run_sink_resumes_from_persisted_cursor() {
    let all: Vec<_> = (1..=10).map(|seq| make_receipt(seq, ReceiptEvent::RunStart, json!({}))).collect();
    let cursors = Arc::new(FakeCursorStore::new());

    let first_source = FakeLogSource::new(all[..5].to_vec());
    let mut sink = FakeSink::new("resume");
    let (tx, mut rx) = watch::channel(false);
    let cursors_clone = Arc::clone(&cursors);
    let handle = tokio::spawn(async move {
        run_sink(&first_source, cursors_clone.as_ref(), &mut sink, &fast_config(), &mut rx).await
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    let _ = tx.send(true);
    handle.await.unwrap().unwrap();

    let second_source = FakeLogSource::new(all);
    let mut sink2 = FakeSink::new("resume");
    let delivered = sink2.delivered_handle();
    let (tx2, mut rx2) = watch::channel(false);
    let handle2 = tokio::spawn(async move {
        run_sink(&second_source, cursors.as_ref(), &mut sink2, &fast_config(), &mut rx2).await
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    let _ = tx2.send(true);
    handle2.await.unwrap().unwrap();

    let mut seen = delivered.lock().unwrap().clone();
    seen.sort_unstable();
    assert_eq!(seen, vec![6, 7, 8, 9, 10]);
}
