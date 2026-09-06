//! Dev-only structured event log — the relay-rs side of the unified dev-logs/ pipeline.
//!
//! Deliberately kept out of `session.rs`: that module is a pure state machine asserted by exact
//! `Action` equality in its tests (AGENTS.md determinism invariant), so it must stay free of I/O
//! and wall-clock reads. This module is called only from `main.rs`'s socket plumbing, which is
//! already the impure layer — logging the frame in / action out and its latency there captures
//! the same information without touching the pure core.
//!
//! Best-effort: a logging failure must never break the audio hot path.

use serde_json::{json, Map, Value};
use std::fs::{create_dir_all, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn dev_logging_enabled() -> bool {
    std::env::var("ORB_DEV_LOGGING")
        .map(|v| v != "0")
        .unwrap_or(true)
}

fn log_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("ORB_DEV_LOG_DIR") {
        return PathBuf::from(dir);
    }
    // backend/relay-rs -> product root is 2 parents up from the crate manifest dir.
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../dev-logs")
}

pub fn dev_log(event: &str, level: &str, mut fields: Map<String, Value>) {
    if !dev_logging_enabled() {
        return;
    }
    let dir = log_dir();
    if create_dir_all(&dir).is_err() {
        return;
    }
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    fields.insert("ts".into(), json!(ts));
    fields.insert("service".into(), json!("relay-rs"));
    fields.insert("level".into(), json!(level));
    fields.insert("event".into(), json!(event));
    let Ok(line) = serde_json::to_string(&Value::Object(fields)) else {
        return;
    };
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("relay-rs.ndjson"))
    {
        let _ = writeln!(file, "{line}");
    }
}

pub fn truncate(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_string();
    }
    let head: String = value.chars().take(limit).collect();
    format!("{head}…(+{} chars)", value.chars().count() - limit)
}
