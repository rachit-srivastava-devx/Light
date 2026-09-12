use ingest::{normalize, IncomingEvent, IngestError, SourceRegistration, RedactedCategory};
use serde_json::json;

fn reg() -> SourceRegistration { SourceRegistration { namespace: "src".into(), schema_version: 1 } }
fn ev(payload: serde_json::Value) -> IncomingEvent {
    IncomingEvent { source: "src".into(), delivery_id: "d1".into(), payload, attachments: vec![] }
}

#[test] fn api_key_is_scrubbed() {
    let out = normalize(ev(json!({"api_key": "sk-abcdefghijklmnopqrst"})), &reg()).unwrap();
    assert!(out.payload["api_key"].as_str().unwrap().contains("REDACTED"));
    assert_eq!(out.redaction_receipt.field_count, 1);
    // sk-... pattern → ApiKey, not GenericSecret (field name is a hint, value pattern wins)
    assert!(out.redaction_receipt.categories.contains(&RedactedCategory::ApiKey));
}

#[test] fn bearer_token_is_scrubbed() {
    let out = normalize(ev(json!({"header": "Bearer eyJhbGciOiJSUzI1NiJ9"})), &reg()).unwrap();
    assert!(out.payload["header"].as_str().unwrap().contains("REDACTED"));
    assert!(out.redaction_receipt.categories.contains(&RedactedCategory::BearerToken));
}

#[test] fn pem_key_is_scrubbed() {
    let out = normalize(ev(json!({"key": "-----BEGIN RSA PRIVATE KEY-----"})), &reg()).unwrap();
    assert!(out.payload["key"].as_str().unwrap().contains("REDACTED"));
    assert!(out.redaction_receipt.categories.contains(&RedactedCategory::PrivateKey));
}

#[test] fn secret_field_name_is_scrubbed() {
    let out = normalize(ev(json!({"password": "hunter2"})), &reg()).unwrap();
    assert_eq!(out.payload["password"].as_str().unwrap(), "[REDACTED:GenericSecret]");
    assert_eq!(out.redaction_receipt.field_count, 1);
}

#[test] fn non_string_value_under_secret_field_is_redacted() {
    // D1: {"password": ["hunter2"]} must not pass through unredacted.
    let out = normalize(ev(json!({"password": ["hunter2", "another"]})), &reg()).unwrap();
    assert_eq!(out.payload["password"].as_str().unwrap(), "[REDACTED:GenericSecret]");
}

#[test] fn nested_secret_field_is_scrubbed() {
    let out = normalize(ev(json!({"a": {"b": {"password": "s3cr3t"}}})), &reg()).unwrap();
    assert!(out.payload["a"]["b"]["password"].as_str().unwrap().contains("REDACTED"));
}

#[test] fn clean_payload_has_empty_receipt() {
    let out = normalize(ev(json!({"title": "fix login bug", "count": 42})), &reg()).unwrap();
    assert_eq!(out.redaction_receipt.field_count, 0);
    assert!(out.redaction_receipt.categories.is_empty());
}

#[test] fn json_too_deep_refuses() {
    let mut v = json!("leaf");
    for _ in 0..34 { v = json!({"k": v}); }
    assert_eq!(normalize(ev(v), &reg()).unwrap_err(), IngestError::JsonTooDeep);
}

#[test] fn json_too_complex_refuses() {
    // 10,001 leaves of "x" ≈ 40KB — under size limit but exceeds MAX_JSON_LEAVES (10,000).
    let leaves: Vec<serde_json::Value> = (0..=10_000).map(|_| json!("x")).collect();
    assert_eq!(normalize(ev(json!(leaves)), &reg()).unwrap_err(), IngestError::JsonTooComplex);
}
