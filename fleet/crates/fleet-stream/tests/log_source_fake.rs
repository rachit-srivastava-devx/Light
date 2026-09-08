//! Shared in-memory `LogSource`/`CursorStore` test fakes for `fleet-stream`'s own suite.
#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Mutex;

use fleet_types::{Blake3Hash, PrevHash, Receipt, ReceiptEvent, SchemaV1};
use fleet_stream::{CursorError, CursorStore, LogSource, LogSourceError};

pub fn make_receipt(seq: u64, event: ReceiptEvent, body: serde_json::Value) -> Receipt {
    Receipt {
        schema_version: SchemaV1,
        seq,
        prev_hash: PrevHash::Genesis,
        hash: Blake3Hash::parse(format!("blake3:{seq:064x}")).unwrap(),
        ts_wall: "2026-01-01T00:00:00Z".to_string(),
        event,
        actor: "test-actor".to_string(),
        resolved_model: None,
        exit_code: None,
        body,
    }
}

/// A source backed by a fixed in-memory `Vec<Receipt>`. `poll_since` never mutates it, matching
/// `LogSource`'s statelessness contract.
pub struct FakeLogSource {
    receipts: Vec<Receipt>,
}

impl FakeLogSource {
    pub fn new(receipts: Vec<Receipt>) -> Self {
        Self { receipts }
    }
}

impl LogSource for FakeLogSource {
    fn poll_since(&self, after: Option<u64>) -> Result<Vec<Receipt>, LogSourceError> {
        Ok(self
            .receipts
            .iter()
            .filter(|r| after.map(|a| r.seq > a).unwrap_or(true))
            .cloned()
            .collect())
    }
}

#[derive(Default)]
pub struct FakeCursorStore {
    cursors: Mutex<HashMap<&'static str, u64>>,
}

impl FakeCursorStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl CursorStore for FakeCursorStore {
    fn load(&self, sink_id: &'static str) -> Result<Option<u64>, CursorError> {
        Ok(self.cursors.lock().unwrap().get(sink_id).copied())
    }

    fn save(&self, sink_id: &'static str, seq: u64) -> Result<(), CursorError> {
        self.cursors.lock().unwrap().insert(sink_id, seq);
        Ok(())
    }
}
