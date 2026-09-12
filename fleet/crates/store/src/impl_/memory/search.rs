//! `bm25_search()`, `vector_search()`, `count()` -- the two raw, unfused ranked reads plus a
//! row count. `memory_store.py:326-346` (bm25 half of `search`), `:99-100` (`count`).

use rusqlite::params;

use super::types::{Bm25Hit, MemoryError, Severity, VectorHit};
use super::MemoryStore;

impl MemoryStore {
    /// Raw BM25 match over pre-tokenized terms, unranked against anything but BM25 itself.
    pub fn bm25_search(&self, terms: &[String], limit: u32) -> Result<Vec<Bm25Hit>, MemoryError> {
        if terms.is_empty() {
            return Ok(Vec::new());
        }
        let match_expr = terms
            .iter()
            .map(|t| format!("\"{}\"", t.replace('"', "\"\"")))
            .collect::<Vec<_>>()
            .join(" OR ");
        let mut stmt = self.conn.prepare(
            "SELECT m.id, m.source, m.title, m.severity, m.confirmed_count,
                    round(-bm25(memories_fts, 5.0, 2.0, 1.0, 1.0), 6) AS relevance
             FROM memories_fts JOIN memories AS m ON m.rowid = memories_fts.rowid
             WHERE memories_fts MATCH ?1
             ORDER BY bm25(memories_fts, 5.0, 2.0, 1.0, 1.0), m.updated_at DESC, m.id
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![match_expr, limit], |row| {
            Ok(Bm25Hit {
                id: row.get(0)?,
                source: row.get(1)?,
                title: row.get(2)?,
                severity: Severity::parse(&row.get::<_, String>(3)?),
                confirmed_count: row.get::<_, i64>(4)? as u32,
                relevance: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(MemoryError::from)
    }

    /// Raw vector k-NN, unranked against anything but distance.
    pub fn vector_search(&self, query: &[f32], limit: u32) -> Result<Vec<VectorHit>, MemoryError> {
        if query.len() as u32 != self.vector_dimensions {
            return Err(MemoryError::DimensionMismatch {
                expected: self.vector_dimensions,
                found: query.len() as u32,
            });
        }
        let bytes: Vec<u8> = query.iter().flat_map(|v| v.to_le_bytes()).collect();
        let mut stmt = self.conn.prepare(
            "SELECT m.id, v.distance FROM memory_vectors v JOIN memories m ON m.rowid = v.rowid
             WHERE v.embedding MATCH ?1 AND k = ?2 ORDER BY v.distance",
        )?;
        let rows = stmt.query_map(params![bytes, limit], |row| {
            Ok(VectorHit { id: row.get(0)?, distance: row.get(1)? })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(MemoryError::from)
    }

    /// Row count.
    pub fn count(&self) -> Result<u64, MemoryError> {
        let n: i64 = self.conn.query_row("SELECT count(*) FROM memories", [], |r| r.get(0))?;
        Ok(n as u64)
    }
}
