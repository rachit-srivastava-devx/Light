//! `KvError`.

use crate::io_fault::IoFault;

#[derive(Debug, thiserror::Error)]
pub enum KvError {
    #[error(transparent)]
    Io(#[from] IoFault),
    #[error("redb: {0}")]
    Redb(String),
}

impl From<redb::Error> for KvError {
    fn from(e: redb::Error) -> Self {
        KvError::Redb(e.to_string())
    }
}

impl From<redb::TransactionError> for KvError {
    fn from(e: redb::TransactionError) -> Self {
        KvError::Redb(e.to_string())
    }
}

impl From<redb::TableError> for KvError {
    fn from(e: redb::TableError) -> Self {
        KvError::Redb(e.to_string())
    }
}

impl From<redb::StorageError> for KvError {
    fn from(e: redb::StorageError) -> Self {
        KvError::Redb(e.to_string())
    }
}

impl From<redb::CommitError> for KvError {
    fn from(e: redb::CommitError) -> Self {
        KvError::Redb(e.to_string())
    }
}

impl From<redb::DatabaseError> for KvError {
    fn from(e: redb::DatabaseError) -> Self {
        KvError::Redb(e.to_string())
    }
}
