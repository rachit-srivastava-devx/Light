use super::super::normalize;
use crate::{IncomingEvent, SourceRegistration};
use serde_json::json;

#[test]
fn accepts_the_inclusive_payload_size_limit() {
    let payload = serde_json::Value::String("x".repeat(262_142));
    let input = IncomingEvent {
        source: "src".into(),
        delivery_id: "size".into(),
        payload,
        attachments: vec![],
    };
    assert!(normalize(
        input,
        &SourceRegistration {
            namespace: "src".into(),
            schema_version: 1
        }
    )
    .is_ok());
}

#[test]
fn digest_is_blake3_of_the_scrubbed_json() {
    let payload = json!({"body": "safe"});
    let out = normalize(
        IncomingEvent {
            source: "src".into(),
            delivery_id: "digest".into(),
            payload: payload.clone(),
            attachments: vec![],
        },
        &SourceRegistration {
            namespace: "src".into(),
            schema_version: 1,
        },
    )
    .unwrap();
    let expected = format!(
        "blake3:{}",
        blake3::hash(&serde_json::to_vec(&payload).unwrap()).to_hex()
    );
    assert_eq!(out.payload_digest, expected);
}
