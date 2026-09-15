use crate::{
    normalize_authenticated, AttachmentRef, AuthEvidence, AuthenticatedIncomingEvent,
    ConnectorEnvelope, IngestError, NormalizedEvent, SourceRegistration, MAX_PAYLOAD_REF_LEN,
};
use serde_json::Value;

pub fn normalize_connector(
    envelope: ConnectorEnvelope,
    payload: Value,
    attachments: Vec<AttachmentRef>,
    reg: &SourceRegistration,
) -> Result<NormalizedEvent, IngestError> {
    let bad_ref = envelope.payload_ref.is_empty()
        || envelope.payload_ref.len() > MAX_PAYLOAD_REF_LEN
        || envelope.payload_ref.chars().any(char::is_control);
    if bad_ref {
        return Err(IngestError::InvalidPayloadRef);
    }
    if envelope.schema_version == 0
        || envelope.schema_version != u64::from(reg.schema_version)
        || envelope.payload_digest.is_empty()
    {
        return Err(IngestError::InvalidSchemaVersion);
    }
    let mut event = normalize_authenticated(
        AuthenticatedIncomingEvent {
            source: envelope.source,
            delivery_id: envelope.delivery_id,
            object_version: envelope.object_version,
            object_version_position: None,
            payload,
            attachments,
            auth: AuthEvidence {
                external_actor: envelope.actor,
                auth_metadata_ref: envelope.auth_metadata_ref,
            },
            cursor: None,
            cursor_position: None,
        },
        reg,
    )?;
    if event.payload_digest != envelope.payload_digest {
        return Err(IngestError::PayloadDigestMismatch);
    }
    event.payload_ref = Some(envelope.payload_ref);
    Ok(event)
}

#[cfg(test)]
mod tests;
