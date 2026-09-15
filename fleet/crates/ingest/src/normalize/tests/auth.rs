use super::super::normalize_authenticated;
use crate::{AuthEvidence, AuthenticatedIncomingEvent, IngestError, SourceRegistration};
use serde_json::json;

fn valid() -> AuthenticatedIncomingEvent {
    AuthenticatedIncomingEvent {
        source: "src".into(),
        delivery_id: "delivery".into(),
        object_version: "v".into(),
        object_version_position: None,
        payload: json!({"ok": true}),
        attachments: vec![],
        auth: AuthEvidence {
            external_actor: "actor".into(),
            auth_metadata_ref: "ref".into(),
        },
        cursor: None,
        cursor_position: None,
    }
}

fn assert_error(mutate: fn(&mut AuthenticatedIncomingEvent), expected: IngestError) {
    let mut input = valid();
    mutate(&mut input);
    assert_eq!(
        normalize_authenticated(
            input,
            &SourceRegistration {
                namespace: "src".into(),
                schema_version: 1
            }
        )
        .unwrap_err(),
        expected
    );
}

#[test]
fn accepts_each_inclusive_metadata_boundary() {
    let mut input = valid();
    input.object_version = "v".repeat(256);
    input.object_version_position = Some(9_223_372_036_854_775_807);
    input.auth.external_actor = "a".repeat(256);
    input.auth.auth_metadata_ref = "r".repeat(256);
    input.cursor = Some("c".repeat(256));
    input.cursor_position = Some(9_223_372_036_854_775_807);
    assert!(normalize_authenticated(
        input,
        &SourceRegistration {
            namespace: "src".into(),
            schema_version: 1
        }
    )
    .is_ok());
}

#[test]
fn rejects_each_independent_authentication_failure() {
    let cases: [fn(&mut AuthenticatedIncomingEvent); 9] = [
        |i: &mut AuthenticatedIncomingEvent| i.object_version.clear(),
        |i| i.object_version.push('\u{7f}'),
        |i| i.auth.external_actor = " ".into(),
        |i| i.auth.auth_metadata_ref = " ".into(),
        |i| i.auth.external_actor = "a".repeat(257),
        |i| i.auth.auth_metadata_ref = "r".repeat(257),
        |i| i.auth.external_actor.push('\u{7f}'),
        |i| i.auth.auth_metadata_ref.push('\u{7f}'),
        |i| i.object_version_position = Some(9_223_372_036_854_775_808),
    ];
    for mutate in cases {
        assert_error(mutate, IngestError::InvalidAuthEvidence);
    }
}

#[test]
fn rejects_each_independent_cursor_failure() {
    let cases: [fn(&mut AuthenticatedIncomingEvent); 6] = [
        |i: &mut AuthenticatedIncomingEvent| {
            i.cursor = Some(String::new());
            i.cursor_position = Some(0);
        },
        |i| {
            i.cursor = Some("c".repeat(257));
            i.cursor_position = Some(0);
        },
        |i| {
            i.cursor = Some("bad\u{7f}".into());
            i.cursor_position = Some(0);
        },
        |i| i.cursor = Some("cursor".into()),
        |i| i.cursor_position = Some(1),
        |i| {
            i.cursor = Some("cursor".into());
            i.cursor_position = Some(9_223_372_036_854_775_808);
        },
    ];
    for mutate in cases {
        assert_error(mutate, IngestError::InvalidCursor);
    }
}
