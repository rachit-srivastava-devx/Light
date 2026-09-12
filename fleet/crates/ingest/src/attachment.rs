use std::collections::HashSet;
use crate::types::{AttachmentRef, IngestError, MAX_ATTACHMENTS_PER_EVENT, MAX_ATTACHMENT_BYTES, MAX_ATTACHMENT_URI_LEN};

pub fn validate_attachments(refs: &[AttachmentRef]) -> Result<(), IngestError> {
    check_count(refs)?;
    check_no_duplicate_uris(refs)?;
    for r in refs { check_one(r)?; }
    Ok(())
}

fn check_count(refs: &[AttachmentRef]) -> Result<(), IngestError> {
    if refs.len() > MAX_ATTACHMENTS_PER_EVENT { Err(IngestError::TooManyAttachments) } else { Ok(()) }
}

fn check_no_duplicate_uris(refs: &[AttachmentRef]) -> Result<(), IngestError> {
    let mut seen: HashSet<&str> = HashSet::new();
    for r in refs {
        if !seen.insert(r.uri.as_str()) { return Err(IngestError::DuplicateAttachmentUri); }
    }
    Ok(())
}

fn check_one(r: &AttachmentRef) -> Result<(), IngestError> {
    if r.uri.is_empty() || r.digest.is_empty() || r.mime_type.is_empty() {
        return Err(IngestError::InvalidAttachment);
    }
    if r.uri.len() > MAX_ATTACHMENT_URI_LEN {
        return Err(IngestError::AttachmentUriTooLong);
    }
    match r.size_bytes {
        0 => Err(IngestError::InvalidAttachment),
        n if n > MAX_ATTACHMENT_BYTES => Err(IngestError::AttachmentOversized),
        _ => Ok(()),
    }
}
