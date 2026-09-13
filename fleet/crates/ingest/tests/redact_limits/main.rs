use ingest::{normalize, IngestError};
use serde_json::json;

#[path = "../common/mod.rs"]
mod common;
use common::{ev, reg};

#[test]
fn clean_payload_has_empty_receipt() {
    let out = normalize(ev(json!({"title": "fix login bug", "count": 42})), &reg()).unwrap();
    assert_eq!(out.redaction_receipt.field_count, 0);
    assert!(out.redaction_receipt.categories.is_empty());
}

#[test]
fn json_too_deep_refuses() {
    let mut v = json!("leaf");
    for _ in 0..34 {
        v = json!({"k": v});
    }
    assert_eq!(
        normalize(ev(v), &reg()).unwrap_err(),
        IngestError::JsonTooDeep
    );
}

#[test]
fn json_too_complex_refuses() {
    // 10,001 leaves of "x" ≈ 40KB — under size limit but exceeds MAX_JSON_LEAVES (10,000).
    let leaves: Vec<serde_json::Value> = (0..=10_000).map(|_| json!("x")).collect();
    assert_eq!(
        normalize(ev(json!(leaves)), &reg()).unwrap_err(),
        IngestError::JsonTooComplex
    );
}
