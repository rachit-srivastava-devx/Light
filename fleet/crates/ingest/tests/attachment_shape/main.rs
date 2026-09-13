use ingest::{normalize, AttachmentRef, IngestError};

#[path = "../common/mod.rs"]
mod common;
use common::{ev_with, reg};

#[test]
fn attachment_empty_digest_refuses() {
    let r = normalize(
        ev_with(vec![AttachmentRef {
            uri: "s3://x".into(),
            digest: "".into(),
            mime_type: "image/png".into(),
            size_bytes: 1,
        }]),
        &reg(),
    );
    assert_eq!(r.unwrap_err(), IngestError::InvalidAttachment);
}

#[test]
fn attachment_empty_mime_refuses() {
    let r = normalize(
        ev_with(vec![AttachmentRef {
            uri: "s3://x".into(),
            digest: "abc".into(),
            mime_type: "".into(),
            size_bytes: 1,
        }]),
        &reg(),
    );
    assert_eq!(r.unwrap_err(), IngestError::InvalidAttachment);
}

#[test]
fn zero_attachments_passes() {
    let r = normalize(ev_with(vec![]), &reg());
    assert!(r.is_ok(), "{r:?}");
}
