use super::validate_attachments;
use crate::{AttachmentRef, IngestError};

fn valid(uri: String) -> AttachmentRef {
    AttachmentRef {
        uri,
        digest: "digest".into(),
        mime_type: "application/octet-stream".into(),
        size_bytes: 1,
    }
}

#[test]
fn accepts_each_inclusive_attachment_bound() {
    let mut refs: Vec<_> = (0..99).map(|i| valid(format!("s3://bucket/{i}"))).collect();
    refs.push(AttachmentRef {
        uri: "u".repeat(4096),
        digest: "digest".into(),
        mime_type: "application/octet-stream".into(),
        size_bytes: 16_777_216,
    });
    assert!(validate_attachments(&refs).is_ok());
}

#[test]
fn rejects_each_independent_attachment_failure() {
    let cases = [
        AttachmentRef {
            uri: String::new(),
            ..valid("u".into())
        },
        AttachmentRef {
            digest: String::new(),
            ..valid("u".into())
        },
        AttachmentRef {
            mime_type: String::new(),
            ..valid("u".into())
        },
        AttachmentRef {
            size_bytes: 0,
            ..valid("u".into())
        },
        AttachmentRef {
            size_bytes: 16_777_217,
            ..valid("u".into())
        },
        AttachmentRef {
            uri: "u".repeat(4097),
            ..valid("u".into())
        },
    ];
    for item in cases {
        assert!(matches!(
            validate_attachments(&[item]),
            Err(IngestError::InvalidAttachment
                | IngestError::AttachmentOversized
                | IngestError::AttachmentUriTooLong)
        ));
    }
    let duplicate = vec![valid("same".into()), valid("same".into())];
    assert_eq!(
        validate_attachments(&duplicate).unwrap_err(),
        IngestError::DuplicateAttachmentUri
    );
}
