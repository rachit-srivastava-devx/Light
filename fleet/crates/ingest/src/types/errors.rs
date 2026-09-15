#[derive(Debug, thiserror::Error, PartialEq)]
pub enum IngestError {
    #[error("unknown or empty source namespace")]
    UnknownSource,
    #[error("delivery id is empty, oversized, or contains control characters")]
    InvalidDeliveryId,
    #[error("schema version must be non-zero")]
    InvalidSchemaVersion,
    #[error("payload reference is empty, oversized, or contains control characters")]
    InvalidPayloadRef,
    #[error("declared payload digest does not match normalized payload")]
    PayloadDigestMismatch,
    #[error("authenticated event metadata is empty or invalid")]
    InvalidAuthEvidence,
    #[error("cursor must include a bounded position and non-empty value")]
    InvalidCursor,
    #[error("durable store failure: {0}")]
    Store(String),
    #[error("payload exceeds size limit")]
    OversizedPayload,
    #[error("json nesting exceeds depth limit")]
    JsonTooDeep,
    #[error("json has too many string leaves")]
    JsonTooComplex,
    #[error("serialization error: {0}")]
    SerializationError(String),
    #[error("redaction failed: {0}")]
    RedactionFailed(String),
    #[error("invalid attachment field")]
    InvalidAttachment,
    #[error("attachment binary exceeds size limit")]
    AttachmentOversized,
    #[error("too many attachments")]
    TooManyAttachments,
    #[error("duplicate attachment URI within event")]
    DuplicateAttachmentUri,
    #[error("attachment URI exceeds length limit")]
    AttachmentUriTooLong,
}
