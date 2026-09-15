use super::redact_secrets;
use crate::types::MAX_JSON_DEPTH;
use serde_json::json;

#[test]
fn accepts_the_inclusive_depth_limit() {
    let mut value = json!("safe");
    for _ in 0..MAX_JSON_DEPTH {
        value = json!({"nested": value});
    }
    assert!(redact_secrets(value).is_ok());
}

#[test]
fn accepts_the_inclusive_leaf_limit() {
    let leaves: Vec<_> = (0..10_000).map(|_| json!("safe")).collect();
    assert!(redact_secrets(json!(leaves)).is_ok());
}

#[test]
fn accepts_a_secret_field_at_the_inclusive_leaf_limit() {
    let leaves: Vec<_> = (0..9_999).map(|_| json!("safe")).collect();
    let payload = json!([leaves, {"password": "secret"}]);
    assert!(redact_secrets(payload).is_ok());
}
