use super::normalize_connector;
use crate::{
    normalize_authenticated, AuthEvidence, AuthenticatedIncomingEvent, ConnectorEnvelope,
    IngestError, SourceRegistration,
};
use serde_json::json;

fn envelope(payload_ref: String) -> (ConnectorEnvelope, serde_json::Value, SourceRegistration) {
    let payload = json!({"body": "issue"});
    let registration = SourceRegistration {
        namespace: "src".into(),
        schema_version: 1,
    };
    let digest = normalize_authenticated(
        AuthenticatedIncomingEvent {
            source: "src".into(),
            delivery_id: "delivery".into(),
            object_version: "v1".into(),
            object_version_position: None,
            payload: payload.clone(),
            attachments: vec![],
            auth: AuthEvidence {
                external_actor: "actor".into(),
                auth_metadata_ref: "auth".into(),
            },
            cursor: None,
            cursor_position: None,
        },
        &registration,
    )
    .unwrap()
    .payload_digest;
    (
        ConnectorEnvelope {
            source: "src".into(),
            delivery_id: "delivery".into(),
            object_version: "v1".into(),
            schema_version: 1,
            actor: "actor".into(),
            payload_ref,
            payload_digest: digest,
            auth_metadata_ref: "auth".into(),
        },
        payload,
        registration,
    )
}

#[test]
fn accepts_the_inclusive_payload_reference_limit() {
    let (envelope, payload, registration) = envelope("r".repeat(4096));
    assert!(normalize_connector(envelope, payload, vec![], &registration).is_ok());
}

#[test]
fn rejects_each_independent_payload_reference_failure() {
    for payload_ref in [String::new(), "r".repeat(4097), "bad\u{7f}".into()] {
        let (envelope, payload, registration) = envelope(payload_ref);
        assert_eq!(
            normalize_connector(envelope, payload, vec![], &registration).unwrap_err(),
            IngestError::InvalidPayloadRef
        );
    }
}
