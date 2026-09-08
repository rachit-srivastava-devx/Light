//! Fast fabrication of a valid, hash-chained ledger of N rows, without going through
//! `Ledger::append` (which re-reads and re-verifies the *whole* file on every call -- O(n) per
//! append, so building 100k rows that way is O(n^2) and impractically slow). This mirrors the
//! exact canonicalization in `src/ledger/canon.rs` and the hash formula in `src/ledger/append.rs`;
//! `serde_json`'s `Map` here is a `BTreeMap` (no `preserve_order` feature anywhere in the
//! workspace), so key insertion order below does not need to match the source's -- both sort the
//! same way before hashing.
use serde_json::{json, Map, Value};

const GENESIS: &str = "GENESIS";

fn canonical_bytes(seq: u64, prev_hash: &str, ts_wall: &str, body: &Value) -> Vec<u8> {
    let mut map = Map::new();
    map.insert("schema_version".into(), json!("1.0"));
    map.insert("seq".into(), json!(seq));
    map.insert("prev_hash".into(), json!(prev_hash));
    map.insert("ts_wall".into(), json!(ts_wall));
    map.insert("event".into(), json!("run_start"));
    map.insert("actor".into(), json!("bulk"));
    map.insert("resolved_model".into(), Value::Null);
    map.insert("exit_code".into(), Value::Null);
    map.insert("body".into(), body.clone());
    serde_json::to_vec(&Value::Object(map)).unwrap()
}

fn row_hash(seq: u64, prev_hash: &str, ts_wall: &str, body: &Value) -> String {
    let canonical = canonical_bytes(seq, prev_hash, ts_wall, body);
    let mut input = prev_hash.as_bytes().to_vec();
    input.extend_from_slice(&canonical);
    format!("blake3:{}", blake3::hash(&input).to_hex())
}

/// Build `n` valid, chained rows as the exact `Value` shape `Ledger` reads/writes (JSONL, one
/// object per row). Row `i`'s body is `{"i": i}` so a specific row is easy to identify/tamper.
pub fn build_chain(n: u64) -> Vec<Value> {
    let ts_wall = "2026-01-01T00:00:00Z";
    let mut rows = Vec::with_capacity(n as usize);
    let mut prev = GENESIS.to_string();
    for seq in 0..n {
        let body = json!({ "i": seq });
        let hash = row_hash(seq, &prev, ts_wall, &body);
        rows.push(json!({
            "schema_version": "1.0",
            "seq": seq,
            "prev_hash": prev,
            "hash": hash,
            "ts_wall": ts_wall,
            "event": "run_start",
            "actor": "bulk",
            "body": body,
        }));
        prev = hash;
    }
    rows
}
