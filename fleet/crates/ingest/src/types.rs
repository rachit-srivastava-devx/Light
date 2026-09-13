use serde_json::Value;

pub const MAX_PAYLOAD_BYTES: usize = 256 * 1024;
pub const MAX_SOURCE_LEN: usize = 256;
pub const MAX_ATTACHMENTS_PER_EVENT: usize = 100;
pub const MAX_ATTACHMENT_URI_LEN: usize = 4096;
pub const MAX_ATTACHMENT_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_JSON_DEPTH: usize = 32;
pub const MAX_JSON_LEAVES: usize = 10_000; // reachable within MAX_PAYLOAD_BYTES; 100k is not
pub const MAX_SEEN_IDS: usize = 1_000_000;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AttachmentRef {
    pub uri: String,
    pub digest: String,
    pub mime_type: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum RedactedCategory {
    ApiKey,
    BearerToken,
    PrivateKey,
    GenericSecret,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct RedactionReceipt {
    pub event_id: String,
    pub categories: Vec<RedactedCategory>,
    pub field_count: usize,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct SourceRegistration {
    pub namespace: String,
    pub schema_version: u16,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct IncomingEvent {
    pub source: String,
    pub delivery_id: String,
    pub payload: Value,
    pub attachments: Vec<AttachmentRef>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct NormalizedEvent {
    pub event_id: String,
    pub source: String,
    pub payload: Value,
    pub payload_digest: String,
    pub attachments: Vec<AttachmentRef>,
    pub redaction_receipt: RedactionReceipt,
    pub injection_taint: bool,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum IngestError {
    #[error("unknown or empty source namespace")]
    UnknownSource,
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
