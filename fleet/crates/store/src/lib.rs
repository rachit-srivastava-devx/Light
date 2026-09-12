/// Re-export so callers never need to depend on rusqlite directly.
pub use rusqlite::Connection;

// Fleet-store backward-compat re-exports for src/ (ledger module, Ledger, etc.)
pub use fleet_store::{ledger, Ledger};

pub(crate) mod schema;
pub(crate) mod transaction;
pub(crate) mod artifact;
mod retention;
#[cfg(test)]
mod tests;

pub use artifact::publish_blob;
pub use schema::{migrate, MigrationReport};
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
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|e| StoreError::Io(e.to_string()))?;
    conn.pragma_update(None, "synchronous", "NORMAL")
        .map_err(|e| StoreError::Io(e.to_string()))?;
    Ok(conn)
}
