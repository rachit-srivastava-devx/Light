//! Row-lookup helpers for `prune.rs`: which ids are eligible, in which order. Delete itself (and
//! the file-size helper) live in `prune_delete.rs` to keep both files under the line budget.

use rusqlite::{params, OptionalExtension};

use super::types::{MemoryError, MemoryRow};
use super::MemoryStore;

impl MemoryStore {
    pub(super) fn old_memory_ids(&self, cutoff_epoch: u64) -> Result<Vec<String>, MemoryError> {
        let mut stmt =
            self.conn.prepare("SELECT id FROM memories WHERE strftime('%s', updated_at) <= ?1")?;
        let rows = stmt.query_map(params![cutoff_epoch as i64], |r| r.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(MemoryError::from)
    }

    pub(super) fn oldest_memory_id(&self) -> Result<Option<String>, MemoryError> {
        let mut preferred = self
            .conn
            .query_row("SELECT id FROM memories ORDER BY updated_at, rowid LIMIT 1", [], |r| r.get(0))
            .optional()
            .map_err(MemoryError::from)?;
        if preferred.is_none() {
            preferred = self
                .conn
                .query_row("SELECT id FROM memories ORDER BY rowid LIMIT 1", [], |r| r.get(0))
                .optional()
                .map_err(MemoryError::from)?;
        }
        Ok(preferred)
    }

    pub(super) fn row_by_id(&self, id: &str) -> Result<Option<MemoryRow>, MemoryError> {
        self.conn
            .query_row(
                "SELECT id, title, body, source, keywords, evidence, severity FROM memories WHERE id=?1",
                params![id],
                |r| {
                    let keywords: String = r.get(4)?;
                    Ok(MemoryRow {
                        id: r.get(0)?,
                        title: r.get(1)?,
                        body: r.get(2)?,
                        source: r.get(3)?,
                        keywords: if keywords.is_empty() {
                            Vec::new()
                        } else {
                            keywords.split(',').map(str::to_string).collect()
                        },
                        evidence: r.get(5)?,
                        severity: super::types::Severity::parse(&r.get::<_, String>(6)?),
                    })
                },
            )
            .optional()
            .map_err(MemoryError::from)
    }
}
