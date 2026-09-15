use crate::{
    attachment, injection, redact, registry, AuthenticatedIncomingEvent, IncomingEvent,
    IngestError, NormalizedEvent, SourceRegistration,
};
use serde_json::Value;
use std::io::{self, Write};

struct ByteCounter(usize);

impl Write for ByteCounter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0 += buf.len();
        if self.0 > crate::MAX_PAYLOAD_BYTES {
            return Err(io::Error::other("oversized"));
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn check_size(payload: &Value) -> Result<(), IngestError> {
    serde_json::to_writer(ByteCounter(0), payload).map_err(|_| IngestError::OversizedPayload)
}

fn compute_digest(payload: &Value) -> Result<String, IngestError> {
    let bytes = serde_json::to_vec(payload)
        .map_err(|error| IngestError::SerializationError(error.to_string()))?;
    Ok(format!("blake3:{}", blake3::hash(&bytes).to_hex()))
}

pub fn normalize(
    input: IncomingEvent,
    reg: &SourceRegistration,
) -> Result<NormalizedEvent, IngestError> {
    let source = registry::check_source(&input.source, reg)?;
    if reg.schema_version == 0 {
        return Err(IngestError::InvalidSchemaVersion);
    }
    let delivery_id = registry::check_delivery_id(&input.delivery_id)?;
    check_size(&input.payload)?;
    let taint = injection::check_injection(&input.payload);
    let (payload, mut receipt) = redact::redact_secrets(input.payload)?;
    attachment::validate_attachments(&input.attachments)?;
    let digest = compute_digest(&payload)?;
    let event_id = format!("{source}-{delivery_id}");
    receipt.event_id = event_id.clone();
    Ok(NormalizedEvent {
        event_id,
        source,
        schema_version: reg.schema_version,
        payload_ref: None,
        delivery_id,
        payload,
        payload_digest: digest,
        attachments: input.attachments,
        redaction_receipt: receipt,
        injection_taint: taint,
        object_version: None,
        object_version_position: None,
        auth: None,
        cursor: None,
        cursor_position: None,
    })
}

pub fn normalize_authenticated(
    input: AuthenticatedIncomingEvent,
    reg: &SourceRegistration,
) -> Result<NormalizedEvent, IngestError> {
    if input.object_version.is_empty()
        || input.object_version.len() > crate::MAX_DELIVERY_ID_LEN
        || input.object_version.chars().any(char::is_control)
        || input.auth.external_actor.trim().is_empty()
        || input.auth.auth_metadata_ref.trim().is_empty()
        || input.auth.external_actor.len() > crate::MAX_AUTH_FIELD_LEN
        || input.auth.auth_metadata_ref.len() > crate::MAX_AUTH_FIELD_LEN
        || input.auth.external_actor.chars().any(char::is_control)
        || input.auth.auth_metadata_ref.chars().any(char::is_control)
        || input
            .object_version_position
            .is_some_and(|position| position > crate::MAX_CURSOR_POSITION)
    {
        return Err(IngestError::InvalidAuthEvidence);
    }
    if input.cursor.as_ref().is_some_and(|cursor| {
        cursor.is_empty()
            || cursor.len() > crate::MAX_DELIVERY_ID_LEN
            || cursor.chars().any(char::is_control)
    }) || input.cursor.is_some() != input.cursor_position.is_some()
        || input
            .cursor_position
            .is_some_and(|position| position > crate::MAX_CURSOR_POSITION)
    {
        return Err(IngestError::InvalidCursor);
    }
    let mut event = normalize(
        IncomingEvent {
            source: input.source,
            delivery_id: input.delivery_id,
            payload: input.payload,
            attachments: input.attachments,
        },
        reg,
    )?;
    event.object_version = Some(input.object_version);
    event.object_version_position = input.object_version_position;
    event.auth = Some(input.auth);
    event.cursor = input.cursor;
    event.cursor_position = input.cursor_position;
    Ok(event)
}

#[cfg(test)]
mod tests;
