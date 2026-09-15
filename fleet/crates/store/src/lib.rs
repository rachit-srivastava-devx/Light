mod impl_;

/// Re-export so callers never need to depend on rusqlite directly.
pub use rusqlite::Connection;

// Inlined from fleet-store (ledger module, Ledger, etc.)
pub use impl_::{ledger, Ledger};

pub(crate) mod artifact;
mod retention;
pub(crate) mod schema;
#[cfg(test)]
mod tests;
pub(crate) mod transaction;

pub use artifact::publish_blob;
pub use schema::{ensure_ingest_schema, migrate, MigrationReport};
pub use transaction::SqlStore;

pub type Revision = u64;
pub type Key = String;

#[derive(Debug, Clone)]
pub struct Event {
    pub id: String,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    pub key: String,
    pub revision: Revision,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct Commit {
    pub revision: u64,
    pub event_id: String,
    pub receipt_id: String,
    pub cursor_advanced: bool,
    pub stale_object_version: bool,
}

#[derive(Debug, Clone)]
pub struct IngestAttachment {
    pub uri: String,
    pub digest: String,
    pub mime_type: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct IngestRecord {
    pub event_id: String,
    pub source: String,
    pub schema_version: u16,
    pub payload_ref: Option<String>,
    pub delivery_id: String,
    pub object_version: String,
    pub object_version_position: Option<u64>,
    pub external_actor: String,
    pub auth_metadata_ref: String,
    pub cursor: Option<String>,
    pub cursor_position: Option<u64>,
    pub payload: Vec<u8>,
    pub payload_digest: String,
    pub injection_taint: bool,
    pub redaction_categories: Vec<u8>,
    pub redaction_field_count: u64,
    pub attachments: Vec<IngestAttachment>,
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("stale revision: current={current}")]
    StaleRevision { current: Revision },
    #[error("duplicate event: id={id}")]
    DuplicateEvent { id: String },
    #[error("incomplete blob")]
    IncompleteBlob,
    #[error("sha256 mismatch")]
    DigestMismatch,
    #[error("io: {0}")]
    Io(String),
    #[error("migration: {0}")]
    Migration(String),
    #[error("ingest delivery conflicts with an existing digest")]
    IngestConflict {
        prior_digest: String,
        new_digest: String,
    },
}

pub trait Store {
    fn append_event(&mut self, expected: Revision, event: Event) -> Result<Commit, StoreError>;
    fn load(&self, key: Key) -> Result<Option<Record>, StoreError>;
    fn cas(&mut self, key: Key, expected: Revision, next: Record) -> Result<Revision, StoreError>;
}

/// Open a file-backed connection with WAL mode and NORMAL synchronous.
/// Callers do not need to import rusqlite — use `store::Connection`.
pub fn open(path: &std::path::Path) -> Result<Connection, StoreError> {
    let conn = Connection::open(path).map_err(|e| StoreError::Io(e.to_string()))?;
    conn.busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|e| StoreError::Io(e.to_string()))?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|e| StoreError::Io(e.to_string()))?;
    conn.pragma_update(None, "synchronous", "NORMAL")
        .map_err(|e| StoreError::Io(e.to_string()))?;
    Ok(conn)
}
