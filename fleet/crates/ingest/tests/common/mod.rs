//! Shared test helpers (not a test binary itself -- `tests/common/` is excluded from discovery).
#![allow(dead_code)]

use ingest::{AttachmentRef, IncomingEvent, SourceRegistration};

pub fn reg() -> SourceRegistration {
    SourceRegistration {
        namespace: "src".into(),
        schema_version: 1,
    }
}

pub fn ev(payload: serde_json::Value) -> IncomingEvent {
    IncomingEvent {
        source: "src".into(),
        delivery_id: "d1".into(),
        payload,
        attachments: vec![],
    }
}

pub fn ev_with(attachments: Vec<AttachmentRef>) -> IncomingEvent {
    IncomingEvent {
        source: "src".into(),
        delivery_id: "d1".into(),
        payload: serde_json::json!({}),
        attachments,
    }
}

pub fn valid_ref(uri: &str) -> AttachmentRef {
    AttachmentRef {
        uri: uri.into(),
        digest: "abc123".into(),
        mime_type: "application/pdf".into(),
        size_bytes: 1024,
    }
}
