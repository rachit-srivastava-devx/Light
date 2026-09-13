use ingest::{
    normalize, AttachmentRef, IngestError, MAX_ATTACHMENTS_PER_EVENT, MAX_ATTACHMENT_BYTES,
    MAX_ATTACHMENT_URI_LEN,
};

#[path = "../common/mod.rs"]
mod common;
use common::{ev_with, reg, valid_ref};

#[test]
fn valid_attachment_passes() {
    let r = normalize(ev_with(vec![valid_ref("s3://bucket/file.pdf")]), &reg());
    assert!(r.is_ok(), "{r:?}");
}

#[test]
fn attachment_empty_uri_refuses() {
    let r = normalize(
        ev_with(vec![AttachmentRef {
            uri: "".into(),
            digest: "x".into(),
            mime_type: "image/png".into(),
            size_bytes: 1,
        }]),
        &reg(),
    );
    assert_eq!(r.unwrap_err(), IngestError::InvalidAttachment);
}

#[test]
fn attachment_uri_too_long_refuses() {
    let long_uri = "a".repeat(MAX_ATTACHMENT_URI_LEN + 1);
    let r = normalize(
        ev_with(vec![AttachmentRef {
            uri: long_uri,
            digest: "x".into(),
            mime_type: "image/png".into(),
            size_bytes: 1,
        }]),
        &reg(),
    );
    assert_eq!(r.unwrap_err(), IngestError::AttachmentUriTooLong);
}

#[test]
fn attachment_zero_size_refuses() {
    let r = normalize(
        ev_with(vec![AttachmentRef {
            uri: "s3://x".into(),
            digest: "x".into(),
            mime_type: "image/png".into(),
            size_bytes: 0,
        }]),
        &reg(),
    );
    assert_eq!(r.unwrap_err(), IngestError::InvalidAttachment);
}

#[test]
fn attachment_oversized_refuses() {
    let r = normalize(
        ev_with(vec![AttachmentRef {
            uri: "s3://x".into(),
            digest: "x".into(),
            mime_type: "image/png".into(),
            size_bytes: MAX_ATTACHMENT_BYTES + 1,
        }]),
        &reg(),
    );
    assert_eq!(r.unwrap_err(), IngestError::AttachmentOversized);
}

#[test]
fn too_many_attachments_refuses() {
    let refs = (0..=MAX_ATTACHMENTS_PER_EVENT)
        .map(|i| valid_ref(&format!("s3://b/{i}")))
        .collect();
    let r = normalize(ev_with(refs), &reg());
    assert_eq!(r.unwrap_err(), IngestError::TooManyAttachments);
}

#[test]
fn duplicate_attachment_uri_refuses() {
    let refs = vec![
        valid_ref("s3://bucket/same.pdf"),
        valid_ref("s3://bucket/same.pdf"),
    ];
    let r = normalize(ev_with(refs), &reg());
    assert_eq!(r.unwrap_err(), IngestError::DuplicateAttachmentUri);
}
