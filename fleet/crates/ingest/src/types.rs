use serde_json::Value;

mod connector;
#[cfg(test)]
mod contract_tests;
mod envelope;
mod errors;
pub use connector::ConnectorEnvelope;
pub use envelope::NormalizedEvent;
pub use errors::IngestError;

pub const MAX_PAYLOAD_BYTES: usize = 256 * 1024;
pub const MAX_SOURCE_LEN: usize = 256;
pub const MAX_DELIVERY_ID_LEN: usize = 256;
pub const MAX_AUTH_FIELD_LEN: usize = 256;
pub const MAX_PAYLOAD_REF_LEN: usize = 4096;
pub const MAX_CURSOR_POSITION: u64 = i64::MAX as u64;
pub const MAX_ATTACHMENTS_PER_EVENT: usize = 100;
pub const MAX_ATTACHMENT_URI_LEN: usize = 4096;
pub const MAX_ATTACHMENT_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_JSON_DEPTH: usize = 32;
pub const MAX_JSON_LEAVES: usize = 10_000; // stricter than the draft LLD; preserves authored acceptance coverage
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AuthEvidence {
    pub external_actor: String,
    pub auth_metadata_ref: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct AuthenticatedIncomingEvent {
    pub source: String,
    pub delivery_id: String,
    pub object_version: String,
    pub object_version_position: Option<u64>,
    pub payload: Value,
    pub attachments: Vec<AttachmentRef>,
    pub auth: AuthEvidence,
    pub cursor: Option<String>,
    pub cursor_position: Option<u64>,
}
