//! `FileSink` writes one NDJSON line per event, in a tempdir -- never the repo tree.

#[path = "log_source_fake.rs"]
mod fake;

use std::fs;

use fleet_types::ReceiptEvent;
use fleet_stream::sinks::FileSink;
use fleet_stream::Sink;
use serde_json::{json, Value};

use fake::make_receipt;

#[test]
fn file_sink_writes_one_ndjson_line_per_event_in_a_tempdir() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("events.ndjson");
    let mut sink = FileSink::new(&path, Vec::new());

    for seq in 1..=20u64 {
        let receipt = make_receipt(seq, ReceiptEvent::RunStart, json!({"n": seq}));
        sink.deliver(&fleet_stream::StreamEvent(receipt)).expect("deliver");
    }

    assert_eq!(sink.path(), path.as_path());
    let contents = fs::read_to_string(&path).expect("read back");
    let lines: Vec<&str> = contents.lines().collect();
    assert_eq!(lines.len(), 20);
    for line in lines {
        serde_json::from_str::<Value>(line).expect("valid json line");
    }
}
