//! `Receipt` edge cases split from `receipt_roundtrip.rs` to keep test files within the crate's
//! 80-line-per-file rule (BLUEPRINT.md §9).

use fleet_types::{Blake3Hash, ExitCode, PrevHash, Receipt, ReceiptEvent, SchemaV1};
use serde_json::json;

fn sample_receipt() -> Receipt {
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
fn receipt_event_includes_rollback_as_8th_variant() {
    let json = serde_json::to_string(&ReceiptEvent::Rollback).unwrap();
    assert_eq!(json, "\"rollback\"");
    assert_eq!(
        serde_json::from_str::<ReceiptEvent>("\"rollback\"").unwrap(),
        ReceiptEvent::Rollback
    );
}

#[test]
fn none_fields_round_trip_as_absent_and_as_null() {
    let mut receipt = sample_receipt();
    receipt.resolved_model = None;
    receipt.exit_code = None;
    let serialized = serde_json::to_string(&receipt).unwrap();
    assert!(!serialized.contains("resolved_model"));
    assert!(!serialized.contains("exit_code"));

    let round_tripped: Receipt = serde_json::from_str(&serialized).unwrap();
    assert_eq!(round_tripped, receipt);

    let with_null = serialized.replace(
        "\"body\":{\"note\":\"hello\"}",
        "\"resolved_model\":null,\"exit_code\":null,\"body\":{\"note\":\"hello\"}",
    );
    let from_null: Receipt = serde_json::from_str(&with_null).unwrap();
    assert_eq!(from_null.resolved_model, None);
    assert_eq!(from_null.exit_code, None);
}

#[test]
fn schema_version_rejects_any_string_but_one_dot_zero() {
    assert!(serde_json::from_str::<SchemaV1>("\"1.0\"").is_ok());
    assert!(serde_json::from_str::<SchemaV1>("\"2.0\"").is_err());
}

#[test]
fn duplicate_seq_keys_reject_not_silently_absorb() {
    // A derived `Deserialize` (unlike a bare `serde_json::Value` map) rejects a duplicate
    // struct field outright rather than silently taking the last value -- this test pins that
    // behavior so a future serde upgrade changing it is caught, not silently absorbed.
    let payload = r#"{"schema_version":"1.0","seq":1,"seq":2,"prev_hash":"GENESIS",
        "hash":"blake3:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "ts_wall":"2026-09-08T00:00:00Z","event":"run_start","actor":"fleet","body":{}}"#;
    assert!(serde_json::from_str::<Receipt>(payload).is_err());
}
