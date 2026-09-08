//! `MemoryStore::usage()`/`MemoryStore::prune()` -- size reporting and row pruning.
//!
//! Memory rows carry an `updated_at` timestamp (`datetime('now')`, UTC), so `max_age_secs` is
//! representable here and requires an injected clock. `max_rows` bounds the `memories` table and
//! `max_bytes` bounds the sqlite file size. Because there is no DELETE trigger, each removed
//! `memories` row is also removed from its FTS5 shadow index, vector table, and vector meta (row
//! lookup/delete helpers live in `prune_rows.rs` to keep this file under the line budget).

use std::time::SystemTime;

use super::types::MemoryError;
use super::MemoryStore;
use crate::retention::{PruneReport, RetentionPolicy, UsageReport};

impl MemoryStore {
    /// Current number of memories and the sqlite file's byte size.
    pub fn usage(&self) -> Result<UsageReport, MemoryError> {
        let row_count: i64 = self.conn.query_row("SELECT count(*) FROM memories", [], |r| r.get(0))?;
        Ok(UsageReport { row_count: row_count as u64, byte_size: self.file_bytes()? })
    }

    /// Delete memories (and their dependent rows) until every applicable limit in `policy` is
    /// satisfied. The injected `now` anchors `max_age_secs`; rows older than the cutoff are
    /// removed first, then row-count and byte limits are enforced on the remainder.
    pub fn prune(&mut self, policy: &RetentionPolicy, now: SystemTime) -> Result<PruneReport, MemoryError> {
        let before = self.file_bytes()?;
        let mut removed = 0u64;
        if let Some(max_age) = policy.max_age_secs {
            let cutoff = now
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs().saturating_sub(max_age))
                .unwrap_or(0);
            let ids = self.old_memory_ids(cutoff)?;
            for id in ids {
                if let Some(row) = self.row_by_id(&id)? {
                    removed += 1;
                    self.delete_memory(&id, row)?;
                }
            }
        }
        loop {
            let usage = self.usage()?;
            let over_rows = policy.max_rows.is_some_and(|m| usage.row_count > m);
            let over_bytes = policy.max_bytes.is_some_and(|m| usage.byte_size > m);
            if !over_rows && !over_bytes {
                break;
            }
            let Some(id) = self.oldest_memory_id()? else { break };
            if let Some(row) = self.row_by_id(&id)? {
                self.delete_memory(&id, row)?;
                removed += 1;
            }
        }
        let after = self.file_bytes()?;
        let reclaimed = before.saturating_sub(after);
        Ok(PruneReport { rows_removed: removed, bytes_reclaimed: reclaimed })
    }
}
