//! `GraphStore::usage()`/`GraphStore::prune()` -- size reporting and whole-project pruning.
//!
//! The graph schema has no timestamped column, so `max_age_secs` is not representable here and
//! contributes nothing to a prune. Growth is bounded by `max_rows` (projects) and `max_bytes`
//! (sqlite file size); each bound is enforced at whole-project granularity so no project is ever
//! left with dangling rows in its child tables.

use std::fs;

use rusqlite::{params, OptionalExtension};

use super::types::GraphError;
use super::GraphStore;
use crate::io_fault::IoFault;
use crate::retention::{PruneReport, RetentionPolicy, UsageReport};

impl GraphStore {
    /// Current number of projects and the sqlite file's byte size.
    pub fn usage(&self) -> Result<UsageReport, GraphError> {
        let row_count: i64 = self.conn.query_row("SELECT count(*) FROM projects", [], |r| r.get(0))?;
        Ok(UsageReport { row_count: row_count as u64, byte_size: self.file_bytes()? })
    }

    /// Delete whole projects (and only whole projects) until every applicable limit in `policy`
    /// is satisfied. `max_age_secs` is ignored: no timestamped column exists to age against.
    pub fn prune(&mut self, policy: &RetentionPolicy) -> Result<PruneReport, GraphError> {
        let before = self.file_bytes()?;
        let mut removed = 0u64;
        loop {
            let usage = self.usage()?;
            let over_rows = policy.max_rows.is_some_and(|m| usage.row_count > m);
            let over_bytes = policy.max_bytes.is_some_and(|m| usage.byte_size > m);
            if !over_rows && !over_bytes {
                break;
            }
            let Some(id) = self.oldest_project()? else { break };
            removed += 1;
            self.delete_project(&id)?;
        }
        let after = self.file_bytes()?;
        let reclaimed = before.saturating_sub(after);
        Ok(PruneReport { rows_removed: removed, bytes_reclaimed: reclaimed })
    }

    fn oldest_project(&self) -> Result<Option<String>, GraphError> {
        self.conn
            .query_row("SELECT project_id FROM projects ORDER BY project_id LIMIT 1", [], |r| r.get(0))
            .optional()
            .map_err(GraphError::from)
    }

    fn delete_project(&mut self, id: &str) -> Result<(), GraphError> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM edges WHERE project_id=?1", params![id])?;
        tx.execute("DELETE FROM files WHERE project_id=?1", params![id])?;
        tx.execute("DELETE FROM symbols WHERE project_id=?1", params![id])?;
        tx.execute("DELETE FROM aliases WHERE project_id=?1", params![id])?;
        tx.execute("DELETE FROM projects WHERE project_id=?1", params![id])?;
        tx.commit()?;
        Ok(())
    }

    fn file_bytes(&self) -> Result<u64, GraphError> {
        if !self.db_path.exists() {
            return Ok(0);
        }
        Ok(fs::metadata(&self.db_path)
            .map_err(|e| IoFault::Read { path: self.db_path.clone(), source: e })?
            .len())
    }
}
