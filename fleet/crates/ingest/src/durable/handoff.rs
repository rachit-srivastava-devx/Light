use store::Revision;

#[derive(Debug, Clone, PartialEq)]
pub struct CommittedEventRef {
    pub event_id: String,
    pub revision: Revision,
    pub receipt_id: String,
    pub cursor: Option<String>,
    pub injection_taint: bool,
    pub cursor_advanced: bool,
    pub stale_object_version: bool,
}

/// Metadata-only handoff; raw payload, redaction categories, and attachment bodies are absent.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ControlEventRef {
    pub event_id: String,
    pub revision: Revision,
    pub receipt_id: String,
    pub cursor: Option<String>,
    pub injection_taint: bool,
    pub stale_object_version: bool,
}

impl CommittedEventRef {
    pub fn control_ref(&self) -> ControlEventRef {
        ControlEventRef {
            event_id: self.event_id.clone(),
            revision: self.revision,
            receipt_id: self.receipt_id.clone(),
            cursor: self.cursor.clone(),
            injection_taint: self.injection_taint,
            stale_object_version: self.stale_object_version,
        }
    }
}
