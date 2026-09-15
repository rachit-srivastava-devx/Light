use crate::{IngestError, NormalizedEvent};
use store::{IngestAttachment, IngestRecord, Revision, SqlStore};

mod handoff;
pub use handoff::{CommittedEventRef, ControlEventRef};

pub trait DurableInbox {
    fn accept(&mut self, event: NormalizedEvent) -> Result<crate::InboxDecision, IngestError>;
    fn refuse(
        &self,
        source: &str,
        delivery_id: &str,
        error: &IngestError,
    ) -> Result<(), IngestError>;
    fn committed_cursor(&self, source: &str) -> Result<Option<String>, IngestError>;
    fn committed_event_ref(&self) -> Option<&CommittedEventRef>;
}

pub struct SqlDurableInbox {
    store: SqlStore,
    revision: Revision,
    last_commit: Option<CommittedEventRef>,
}

impl SqlDurableInbox {
    pub fn new(store: SqlStore, revision: Revision) -> Self {
        Self {
            store,
            revision,
            last_commit: None,
        }
    }

    pub fn into_store(self) -> SqlStore {
        self.store
    }
}

impl DurableInbox for SqlDurableInbox {
    fn accept(&mut self, event: NormalizedEvent) -> Result<crate::InboxDecision, IngestError> {
        self.last_commit = None;
        let object_version = event
            .object_version
            .clone()
            .ok_or(IngestError::InvalidAuthEvidence)?;
        let auth = event.auth.clone().ok_or(IngestError::InvalidAuthEvidence)?;
        let payload = serde_json::to_vec(&event.payload)
            .map_err(|e| IngestError::SerializationError(e.to_string()))?;
        let categories = serde_json::to_vec(&event.redaction_receipt.categories)
            .map_err(|e| IngestError::SerializationError(e.to_string()))?;
        let attachments = event
            .attachments
            .into_iter()
            .map(|a| IngestAttachment {
                uri: a.uri,
                digest: a.digest,
                mime_type: a.mime_type,
                size_bytes: a.size_bytes,
            })
            .collect();
        let event_id = event.event_id.clone();
        let cursor = event.cursor.clone();
        let injection_taint = event.injection_taint;
        let record = IngestRecord {
            event_id: event.event_id,
            source: event.source,
            schema_version: event.schema_version,
            payload_ref: event.payload_ref,
            delivery_id: event.delivery_id,
            object_version,
            object_version_position: event.object_version_position,
            external_actor: auth.external_actor,
            auth_metadata_ref: auth.auth_metadata_ref,
            cursor: event.cursor.clone(),
            cursor_position: event.cursor_position,
            payload,
            payload_digest: event.payload_digest,
            injection_taint: event.injection_taint,
            redaction_categories: categories,
            redaction_field_count: event.redaction_receipt.field_count as u64,
            attachments,
        };
        let commit = match self.store.append_ingest(self.revision, record) {
            Ok(commit) => commit,
            Err(store::StoreError::DuplicateEvent { id }) => {
                return Ok(crate::InboxDecision::Duplicate { event_id: id })
            }
            Err(store::StoreError::IngestConflict {
                prior_digest,
                new_digest,
            }) => {
                return Ok(crate::InboxDecision::Conflict {
                    prior_digest,
                    new_digest,
                })
            }
            Err(error) => return Err(IngestError::Store(error.to_string())),
        };
        self.revision = commit.revision;
        self.last_commit = Some(CommittedEventRef {
            event_id: commit.event_id,
            revision: commit.revision,
            receipt_id: commit.receipt_id,
            cursor,
            injection_taint,
            cursor_advanced: commit.cursor_advanced,
            stale_object_version: commit.stale_object_version,
        });
        Ok(crate::InboxDecision::Accepted { event_id })
    }

    fn refuse(
        &self,
        source: &str,
        delivery_id: &str,
        error: &IngestError,
    ) -> Result<(), IngestError> {
        self.store
            .record_ingest_refusal(source, delivery_id, &error.to_string())
            .map_err(|e| IngestError::Store(e.to_string()))
    }

    fn committed_cursor(&self, source: &str) -> Result<Option<String>, IngestError> {
        self.store
            .committed_ingest_cursor(source)
            .map_err(|e| IngestError::Store(e.to_string()))
    }

    fn committed_event_ref(&self) -> Option<&CommittedEventRef> {
        self.last_commit.as_ref()
    }
}
