use ingest::{normalize, AttachmentRef, IncomingEvent, IngestError, SourceRegistration,
             MAX_ATTACHMENTS_PER_EVENT, MAX_ATTACHMENT_BYTES, MAX_ATTACHMENT_URI_LEN};
use serde_json::json;

fn reg() -> SourceRegistration { SourceRegistration { namespace: "src".into(), schema_version: 1 } }

fn ev_with(attachments: Vec<AttachmentRef>) -> IncomingEvent {
    IncomingEvent { source: "src".into(), delivery_id: "d1".into(), payload: json!({}), attachments }
}

fn valid_ref(uri: &str) -> AttachmentRef {
    AttachmentRef { uri: uri.into(), digest: "abc123".into(), mime_type: "application/pdf".into(), size_bytes: 1024 }
}

#[test]
fn valid_attachment_passes() {
    let r = normalize(ev_with(vec![valid_ref("s3://bucket/file.pdf")]), &reg());
    assert!(r.is_ok(), "{r:?}");
}

#[test]
fn attachment_empty_uri_refuses() {
    let r = normalize(ev_with(vec![AttachmentRef { uri: "".into(), digest: "x".into(), mime_type: "image/png".into(), size_bytes: 1 }]), &reg());
    assert_eq!(r.unwrap_err(), IngestError::InvalidAttachment);
}

#[test]
fn attachment_uri_too_long_refuses() {
    let long_uri = "a".repeat(MAX_ATTACHMENT_URI_LEN + 1);
    let r = normalize(ev_with(vec![AttachmentRef { uri: long_uri, digest: "x".into(), mime_type: "image/png".into(), size_bytes: 1 }]), &reg());
    assert_eq!(r.unwrap_err(), IngestError::AttachmentUriTooLong);
}

#[test]
fn attachment_zero_size_refuses() {
    let r = normalize(ev_with(vec![AttachmentRef { uri: "s3://x".into(), digest: "x".into(), mime_type: "image/png".into(), size_bytes: 0 }]), &reg());
    assert_eq!(r.unwrap_err(), IngestError::InvalidAttachment);
}

#[test]
fn attachment_oversized_refuses() {
    let r = normalize(ev_with(vec![AttachmentRef { uri: "s3://x".into(), digest: "x".into(), mime_type: "image/png".into(), size_bytes: MAX_ATTACHMENT_BYTES + 1 }]), &reg());
    assert_eq!(r.unwrap_err(), IngestError::AttachmentOversized);
}

#[test]
fn too_many_attachments_refuses() {
    let refs = (0..=MAX_ATTACHMENTS_PER_EVENT).map(|i| valid_ref(&format!("s3://b/{i}"))).collect();
    let r = normalize(ev_with(refs), &reg());
    assert_eq!(r.unwrap_err(), IngestError::TooManyAttachments);
}

#[test]
fn duplicate_attachment_uri_refuses() {
    let refs = vec![valid_ref("s3://bucket/same.pdf"), valid_ref("s3://bucket/same.pdf")];
    let r = normalize(ev_with(refs), &reg());
    assert_eq!(r.unwrap_err(), IngestError::DuplicateAttachmentUri);
}

#[test]
fn attachment_empty_digest_refuses() {
    let r = normalize(ev_with(vec![AttachmentRef { uri: "s3://x".into(), digest: "".into(), mime_type: "image/png".into(), size_bytes: 1 }]), &reg());
    assert_eq!(r.unwrap_err(), IngestError::InvalidAttachment);
}

#[test]
fn attachment_empty_mime_refuses() {
    let r = normalize(ev_with(vec![AttachmentRef { uri: "s3://x".into(), digest: "abc".into(), mime_type: "".into(), size_bytes: 1 }]), &reg());
    assert_eq!(r.unwrap_err(), IngestError::InvalidAttachment);
}

#[test]
fn zero_attachments_passes() {
    let r = normalize(ev_with(vec![]), &reg());
    assert!(r.is_ok(), "{r:?}");
}
