//! `upsert()`, `set_embedding()`, `embedding_hashes()` -- `memory_store.py:262-313,408-423`.

use rusqlite::params;

use super::types::{MemoryError, MemoryRow};
use super::MemoryStore;

impl MemoryStore {
    /// Insert-or-confirm one row by id: a fresh id inserts at `confirmed_count = 1`; an existing
    /// id updates content and increments `confirmed_count`. Returns the sqlite rowid.
    pub fn upsert(&mut self, row: MemoryRow) -> Result<u64, MemoryError> {
        self.conn.execute(
            "INSERT INTO memories(id, title, body, source, keywords, evidence, severity)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET
               title=excluded.title, body=excluded.body, source=excluded.source,
               keywords=excluded.keywords, evidence=excluded.evidence, severity=excluded.severity,
               confirmed_count=memories.confirmed_count + 1, updated_at=datetime('now')",
            params![
                row.id,
                row.title,
                row.body,
                row.source,
                row.keywords.join(","),
                row.evidence,
                row.severity.as_str(),
            ],
        )?;
        let rowid: i64 =
            self.conn.query_row("SELECT rowid FROM memories WHERE id=?1", params![row.id], |r| r.get(0))?;
        Ok(rowid as u64)
    }

    /// Store (or replace) one row's embedding plus the content-hash it was computed from.
    pub fn set_embedding(&mut self, rowid: u64, vector: &[f32], content_hash: &str) -> Result<(), MemoryError> {
        if vector.len() as u32 != self.vector_dimensions {
            return Err(MemoryError::DimensionMismatch {
                expected: self.vector_dimensions,
                found: vector.len() as u32,
            });
        }
        let bytes: Vec<u8> = vector.iter().flat_map(|v| v.to_le_bytes()).collect();
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM memory_vectors WHERE rowid=?1", params![rowid as i64])?;
        tx.execute(
            "INSERT INTO memory_vectors(rowid, embedding) VALUES (?1, ?2)",
            params![rowid as i64, bytes],
        )?;
        tx.execute(
            "INSERT INTO memory_vector_meta(memory_rowid, content_hash) VALUES (?1, ?2)
             ON CONFLICT(memory_rowid) DO UPDATE SET content_hash=excluded.content_hash",
            params![rowid as i64, content_hash],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// `(rowid, stored_content_hash)` for every row that has a stored embedding.
    pub fn embedding_hashes(&self) -> Result<Vec<(u64, String)>, MemoryError> {
        let mut stmt = self.conn.prepare("SELECT memory_rowid, content_hash FROM memory_vector_meta")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)? as u64, r.get::<_, String>(1)?)))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(MemoryError::from)
    }
}
