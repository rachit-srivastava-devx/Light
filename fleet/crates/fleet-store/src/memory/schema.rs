//! `open()`: pragmas, `CREATE TABLE`/FTS5/triggers, and loading the `vec0` extension in-process
//! (replaces `memory_store.py:197-221`'s subprocess-per-query bridge -- `rusqlite`'s own
//! `load_extension` loads the same compiled artifact directly into this connection).

use std::fs;
use std::path::Path;

use rusqlite::Connection;

use super::types::MemoryError;
use super::MemoryStore;
use crate::io_fault::IoFault;

impl MemoryStore {
    /// Open (creating if absent), ensure `memories`/`memories_fts`/the vector table exist, and
    /// load the `vec0` extension in-process.
    ///
    /// Deviation from BLUEPRINT.md §3: the blueprint's `open(path, vector_dimensions)` has no way
    /// to receive the `vec0` artifact's location, which its own §"Divergence" note (item 4) flags
    /// as an unresolved gap -- injecting a default/discovered path would violate this crate's own
    /// "no ambient IO" rule, so `vec0_extension_path` is added here as a third, caller-supplied
    /// parameter instead.
    pub fn open(path: &Path, vector_dimensions: u32, vec0_extension_path: &Path) -> Result<Self, MemoryError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|source| IoFault::Open { path: path.to_path_buf(), source })?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS memories (
                id TEXT PRIMARY KEY, title TEXT NOT NULL, body TEXT NOT NULL,
                source TEXT NOT NULL DEFAULT '', keywords TEXT NOT NULL DEFAULT '',
                evidence TEXT NOT NULL DEFAULT '', severity TEXT NOT NULL DEFAULT 'important',
                confirmed_count INTEGER NOT NULL DEFAULT 1,
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
             );
             CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
                title, body, keywords, evidence,
                content='memories', content_rowid='rowid', tokenize='porter unicode61'
             );
             CREATE TRIGGER IF NOT EXISTS memories_ai AFTER INSERT ON memories BEGIN
                INSERT INTO memories_fts(rowid, title, body, keywords, evidence)
                VALUES (new.rowid, new.title, new.body, new.keywords, new.evidence);
             END;
             CREATE TRIGGER IF NOT EXISTS memories_au AFTER UPDATE ON memories BEGIN
                INSERT INTO memories_fts(memories_fts, rowid, title, body, keywords, evidence)
                VALUES ('delete', old.rowid, old.title, old.body, old.keywords, old.evidence);
                INSERT INTO memories_fts(rowid, title, body, keywords, evidence)
                VALUES (new.rowid, new.title, new.body, new.keywords, new.evidence);
             END;
             CREATE TABLE IF NOT EXISTS memory_vector_meta (
                memory_rowid INTEGER PRIMARY KEY, content_hash TEXT NOT NULL
             );",
        )?;
        unsafe {
            conn.load_extension_enable()?;
            let result = conn.load_extension(vec0_extension_path, None);
            conn.load_extension_disable()?;
            result?;
        }
        conn.execute(
            &format!("CREATE VIRTUAL TABLE IF NOT EXISTS memory_vectors USING vec0(embedding float[{vector_dimensions}])"),
            [],
        )?;
        Ok(Self { conn, vector_dimensions })
    }
}
