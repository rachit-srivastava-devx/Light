//! Same crash/resume shape as `run_sink_resume_real_file.rs`, extended to the REAL
//! `FileCursorStore` (`src/cursor_file.rs`) in place of `FakeCursorStore`: both the sink's bytes
//! AND the cursor's resume point are durable, on-disk state that must survive a
//! drop-mid-stream-and-restart-on-the-same-paths simulated crash. Reads the cursor file's actual
//! on-disk value back, not just the sink's output.
#[path = "log_source_fake.rs"]
mod fake;
#[path = "support/mod.rs"]
mod support;

use std::sync::Arc;
use std::time::Duration;

use fleet_stream::sinks::FileSink;
use fleet_stream::{run_sink, CursorStore, FileCursorStore};
use fleet_types::ReceiptEvent;
use serde_json::json;
use tokio::sync::watch;

use fake::{make_receipt, FakeLogSource};
use support::fast_config;

#[tokio::test]
async fn real_file_cursor_store_survives_a_simulated_crash_with_no_loss_or_duplication() {
    let dir = tempfile::tempdir().unwrap();
    let events_path = dir.path().join("events.ndjson");
    let cursor_dir = dir.path().join("cursors");
    let all: Vec<_> = (1..=10).map(|seq| make_receipt(seq, ReceiptEvent::RunStart, json!({"i": seq}))).collect();
    let cursors = Arc::new(FileCursorStore::new(&cursor_dir));

    // Run 1: only the first 5 receipts are visible, as if the process crashed right after those
    // were durably appended upstream.
    let first_source = FakeLogSource::new(all[..5].to_vec());
    let mut sink = FileSink::new(&events_path, vec![]);
    let (tx, mut rx) = watch::channel(false);
    let cursors_clone = Arc::clone(&cursors);
    let handle = tokio::spawn(async move {
        run_sink(&first_source, cursors_clone.as_ref(), &mut sink, &fast_config(), &mut rx).await
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    let _ = tx.send(true);
    handle.await.unwrap().unwrap();

    let cursor_after_run1 = cursors.load("file").unwrap();
    assert_eq!(cursor_after_run1, Some(5), "cursor persisted durably before the simulated crash");

    // "Crash": both the sink's file handle and the cursor store handle above are dropped. Run 2
    // opens the SAME paths fresh, as a restarted process would.
    let second_source = FakeLogSource::new(all);
    let mut sink2 = FileSink::new(&events_path, vec![]);
    let cursors2 = Arc::new(FileCursorStore::new(&cursor_dir));
    let (tx2, mut rx2) = watch::channel(false);
    let cursors2_clone = Arc::clone(&cursors2);
    let handle2 = tokio::spawn(async move {
        run_sink(&second_source, cursors2_clone.as_ref(), &mut sink2, &fast_config(), &mut rx2).await
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    let _ = tx2.send(true);
    handle2.await.unwrap().unwrap();

    let text = std::fs::read_to_string(&events_path).unwrap();
    let seqs: Vec<u64> = text
        .lines()
        .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap()["seq"].as_u64().unwrap())
        .collect();
    assert_eq!(seqs, (1..=10).collect::<Vec<_>>(), "no event lost, none duplicated, order preserved");
    assert_eq!(cursors2.load("file").unwrap(), Some(10), "cursor advanced to the true on-disk tip after resume");
}
