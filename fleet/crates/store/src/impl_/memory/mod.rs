//! STATUS: unwired scaffolding matching `docs/LLD/LLD.md` §10/§11 (`DELTA.md` D10, timed
//! P2/P3) — not dead code, no caller yet.
//! The memory substrate's raw storage: rows + FTS5 shadow index + sqlite-vec vector table.
//! `memory_store.py:33-346` (storage half; RRF fusion is `fleet-memory`'s job, not this crate's).

mod prune;
mod prune_delete;
mod prune_rows;
mod schema;
mod search;
mod types;
mod write;

pub use types::{Bm25Hit, MemoryError, MemoryRow, Severity, VectorHit};

pub struct MemoryStore {
    pub(crate) conn: rusqlite::Connection,
    pub(crate) vector_dimensions: u32,
    pub(crate) db_path: std::path::PathBuf,
}
