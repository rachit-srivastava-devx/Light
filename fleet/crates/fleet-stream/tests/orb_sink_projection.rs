//! `OrbSink`'s outbound body matches `contracts/lane-status.v1.json`'s required fields, driven
//! against a real local HTTP listener (no schema library needed -- field presence + type is
//! enough for this closed, small object).

#[path = "log_source_fake.rs"]
mod fake;

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc;

use fleet_types::ReceiptEvent;
use fleet_stream::sinks::OrbSink;
use fleet_stream::{Sink, StreamEvent};
use serde_json::{json, Value};

use fake::make_receipt;

/// A minimal one-shot HTTP/1.1 listener: accepts one POST, captures the body, replies 200.
fn spawn_capture_server() -> (String, mpsc::Receiver<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut buf = [0u8; 8192];
        let mut read = Vec::new();
        loop {
            let n = stream.read(&mut buf).expect("read");
            read.extend_from_slice(&buf[..n]);
            if let Some(header_end) = find_header_end(&read) {
                let headers = String::from_utf8_lossy(&read[..header_end]);
                let len = content_length(&headers);
                if read.len() >= header_end + 4 + len {
                    let body = read[header_end + 4..header_end + 4 + len].to_vec();
                    let _ = tx.send(body);
                    break;
                }
            }
        }
        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
    });
    (format!("http://{addr}/lane-status"), rx)
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

fn content_length(headers: &str) -> usize {
    headers
        .lines()
        .find_map(|line| line.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().to_string()))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

#[test]
fn orb_sink_projection_matches_lane_status_schema() {
    let (url, rx) = spawn_capture_server();
    let mut sink = OrbSink::new(url);
    let receipt = make_receipt(
        42,
        ReceiptEvent::LaneStatus,
        json!({"lane_id": "builder", "role": "builder", "state": "running", "agent": "claude"}),
    );
    sink.deliver(&StreamEvent(receipt)).expect("deliver");

    let body = rx.recv_timeout(std::time::Duration::from_secs(2)).expect("body received");
    let value: Value = serde_json::from_slice(&body).expect("valid json body");
    for field in ["schema_version", "lane_id", "role", "state", "ledger_ref", "actor", "ts_wall"] {
        assert!(value.get(field).is_some(), "missing field {field}");
    }
    assert_eq!(value["schema_version"], "1.0");
    assert_eq!(value["lane_id"], "builder");
    let ledger_ref = &value["ledger_ref"];
    assert_eq!(ledger_ref["seq"], 42);
    assert!(ledger_ref["hash"].as_str().unwrap().starts_with("blake3:"));
}
