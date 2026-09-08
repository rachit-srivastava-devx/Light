//! `delete_memory()`: the multi-table delete that keeps the FTS5 shadow index and vector tables
//! in sync (no DELETE trigger exists for them), plus the file-size helper `prune.rs` reports.

use std::fs;

use rusqlite::{params, OptionalExtension};

use super::types::{MemoryError, MemoryRow};
use super::MemoryStore;
use crate::io_fault::IoFault;

impl MemoryStore {
    pub(super) fn delete_memory(&mut self, id: &str, row: MemoryRow) -> Result<(), MemoryError> {
        let rowid: Option<i64> = self
            .conn
            .query_row("SELECT rowid FROM memories WHERE id=?1", params![id], |r| r.get(0))
            .optional()
            .map_err(MemoryError::from)?;
        let tx = self.conn.transaction()?;
        if let Some(rowid) = rowid {
            tx.execute(
                "INSERT INTO memories_fts(memories_fts, rowid, title, body, keywords, evidence)
                 VALUES ('delete', ?1, ?2, ?3, ?4, ?5)",
                params![rowid, row.title, row.body, row.keywords.join(","), row.evidence],
            )?;
            tx.execute("DELETE FROM memory_vectors WHERE rowid=?1", params![rowid])?;
            tx.execute("DELETE FROM memory_vector_meta WHERE memory_rowid=?1", params![rowid])?;
        }
        tx.execute("DELETE FROM memories WHERE id=?1", params![id])?;
        tx.commit()?;
        Ok(())
    }

    pub(super) fn file_bytes(&self) -> Result<u64, MemoryError> {
        if !self.db_path.exists() {
            return Ok(0);
        }
        Ok(fs::metadata(&self.db_path)
            .map_err(|e| IoFault::Read { path: self.db_path.clone(), source: e })?
            .len())
    }
}
