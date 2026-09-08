//! `run_sink_resumes_from_persisted_cursor` (`run_sink_resume.rs`) proves the pump's resume logic
//! against a `FakeSink` that just records seqs in memory. That leaves the actually-shipped
//! `FileSink` (the only `Sink` impl in `src/sinks/` backed by real, durable IO) unexercised by any
//! crash/resume test. This drives the real `FileSink` writing to a real temp file across a
//! simulated crash (drop the first pump mid-stream, start a second one) and reads the file's
//! actual on-disk bytes back to prove no event is lost or duplicated.
//!
//! See `run_cursor_resume_real_file.rs` for the same shape extended to the real
//! `FileCursorStore`, now that one exists alongside `FileSink`.
#[path = "log_source_fake.rs"]
mod fake;
#[path = "support/mod.rs"]
mod support;

use std::sync::Arc;
use std::time::Duration;

use fleet_stream::sinks::FileSink;
use fleet_stream::run_sink;
use fleet_types::ReceiptEvent;
use serde_json::json;
use tokio::sync::watch;

use fake::{make_receipt, FakeCursorStore, FakeLogSource};
use support::fast_config;

#[tokio::test]
async fn real_file_sink_survives_a_simulated_crash_with_no_loss_or_duplication() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("events.ndjson");
    let all: Vec<_> = (1..=10).map(|seq| make_receipt(seq, ReceiptEvent::RunStart, json!({"i": seq}))).collect();
    let cursors = Arc::new(FakeCursorStore::new());

    // Run 1: only the first 5 receipts are visible to the source, as if the process crashed
    // right after those were durably appended upstream.
    let first_source = FakeLogSource::new(all[..5].to_vec());
    let mut sink = FileSink::new(&path, vec![]);
    let (tx, mut rx) = watch::channel(false);
    let cursors_clone = Arc::clone(&cursors);
    let handle = tokio::spawn(async move {
        run_sink(&first_source, cursors_clone.as_ref(), &mut sink, &fast_config(), &mut rx).await
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    let _ = tx.send(true);
    handle.await.unwrap().unwrap();

    // "Crash": the sink/file-handle above is dropped. Run 2 opens the SAME path fresh, as a
    // restarted process would, and the source now reports the full 10.
    let second_source = FakeLogSource::new(all);
    let mut sink2 = FileSink::new(&path, vec![]);
    let (tx2, mut rx2) = watch::channel(false);
    let handle2 = tokio::spawn(async move {
        run_sink(&second_source, cursors.as_ref(), &mut sink2, &fast_config(), &mut rx2).await
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    let _ = tx2.send(true);
    handle2.await.unwrap().unwrap();

    let text = std::fs::read_to_string(&path).unwrap();
    let seqs: Vec<u64> = text
        .lines()
        .map(|l| serde_json::from_str::<serde_json::Value>(l).unwrap()["seq"].as_u64().unwrap())
        .collect();
    assert_eq!(seqs, (1..=10).collect::<Vec<_>>(), "no event lost, none duplicated, order preserved");
}
