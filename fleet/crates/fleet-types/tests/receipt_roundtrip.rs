//! Serde round-trip against `receipt.v1.json`'s shape (BLUEPRINT.md §9).

use fleet_types::{Blake3Hash, ExitCode, PrevHash, Receipt, ReceiptEvent, SchemaV1};
use serde_json::json;

pub fn sample_receipt() -> Receipt {
    Receipt {
        schema_version: SchemaV1,
        seq: 1,
        prev_hash: PrevHash::Genesis,
        hash: Blake3Hash::parse(format!("blake3:{}", "a".repeat(64))).unwrap(),
        ts_wall: "2026-09-08T00:00:00Z".to_string(),
        event: ReceiptEvent::RunStart,
        actor: "fleet".to_string(),
        resolved_model: Some("claude-sonnet".to_string()),
        exit_code: Some(ExitCode::Ok),
        body: json!({"note": "hello"}),
    }
}

#[test]
fn receipt_round_trips_byte_for_byte() {
    let receipt = sample_receipt();
    let serialized = serde_json::to_string(&receipt).unwrap();
    let deserialized: Receipt = serde_json::from_str(&serialized).unwrap();
    assert_eq!(receipt, deserialized);
}

#[test]
fn huge_seq_and_body_round_trip() {
    let mut receipt = sample_receipt();
    receipt.seq = u64::MAX;
    receipt.body = json!({"blob": "x".repeat(1000)});
    let serialized = serde_json::to_string(&receipt).unwrap();
    let deserialized: Receipt = serde_json::from_str(&serialized).unwrap();
    assert_eq!(receipt, deserialized);
}

#[test]
fn negative_seq_fails_to_deserialize() {
    let bad = r#"{"schema_version":"1.0","seq":-1,"prev_hash":"GENESIS",
        "hash":"blake3:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "ts_wall":"2026-09-08T00:00:00Z","event":"run_start","actor":"fleet","body":{}}"#;
    assert!(serde_json::from_str::<Receipt>(bad).is_err());
}

#[test]
fn unknown_event_fails_to_deserialize_no_catchall() {
    let bad = r#"{"schema_version":"1.0","seq":1,"prev_hash":"GENESIS",
        "hash":"blake3:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "ts_wall":"2026-09-08T00:00:00Z","event":"unknown_event_kind","actor":"fleet","body":{}}"#;
    assert!(serde_json::from_str::<Receipt>(bad).is_err());
}

#[test]
fn unicode_actor_and_body_round_trip() {
    let mut receipt = sample_receipt();
    receipt.actor = "アクター".to_string();
    receipt.body = json!({"emoji": "🚀"});
    let serialized = serde_json::to_string(&receipt).unwrap();
    let deserialized: Receipt = serde_json::from_str(&serialized).unwrap();
    assert_eq!(receipt, deserialized);
}
