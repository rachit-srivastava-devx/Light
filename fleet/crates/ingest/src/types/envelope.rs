use super::{AttachmentRef, AuthEvidence, RedactionReceipt};
use serde_json::Value;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct NormalizedEvent {
    pub event_id: String,
    pub source: String,
    pub schema_version: u16,
    pub payload_ref: Option<String>,
    pub delivery_id: String,
    pub payload: Value,
    pub payload_digest: String,
    pub attachments: Vec<AttachmentRef>,
    pub redaction_receipt: RedactionReceipt,
    pub injection_taint: bool,
    pub object_version: Option<String>,
    pub object_version_position: Option<u64>,
    pub auth: Option<AuthEvidence>,
    pub cursor: Option<String>,
    pub cursor_position: Option<u64>,
}
