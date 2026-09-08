//! The memory substrate's raw storage: rows + FTS5 shadow index + sqlite-vec vector table.
//! `memory_store.py:33-346` (storage half; RRF fusion is `fleet-memory`'s job, not this crate's).

mod schema;
mod search;
mod types;
mod write;

pub use types::{Bm25Hit, MemoryError, MemoryRow, Severity, VectorHit};

pub struct MemoryStore {
    pub(crate) conn: rusqlite::Connection,
    pub(crate) vector_dimensions: u32,
}
