//! `KvStore::usage()`/`KvStore::prune()` -- size reporting and entry pruning.
//!
//! KV entries are generic byte key/value pairs with no timestamp column, so `max_age_secs` is not
//! representable here and contributes nothing to a prune. Growth is bounded by `max_rows` (total
//! entries across all tables) and `max_bytes` (the redb file size); entries are removed in a
//! deterministic per-table, per-key order.

use std::fs;

use redb::{ReadableTable, ReadableTableMetadata, TableDefinition, TableHandle};

use super::types::KvError;
use super::KvStore;
use crate::io_fault::IoFault;
use crate::retention::{PruneReport, RetentionPolicy, UsageReport};

impl KvStore {
    /// Total entry count across all tables and the redb file's byte size.
    pub fn usage(&self) -> Result<UsageReport, KvError> {
        let read_txn = self.db.begin_read()?;
        let mut total = 0u64;
        for handle in read_txn.list_tables()? {
            let def: TableDefinition<&[u8], &[u8]> = TableDefinition::new(handle.name());
            let table = read_txn.open_table(def)?;
            total += table.len()?;
        }
        Ok(UsageReport { row_count: total, byte_size: self.file_bytes()? })
    }

    /// Delete entries until every applicable limit in `policy` is satisfied. `max_age_secs` is
    /// ignored: KV entries carry no timestamp column to age against.
    pub fn prune(&mut self, policy: &RetentionPolicy) -> Result<PruneReport, KvError> {
        let before = self.file_bytes()?;
        let mut keys: Vec<(String, Vec<u8>)> = Vec::new();
        {
            let read_txn = self.db.begin_read()?;
            for handle in read_txn.list_tables()? {
                let def: TableDefinition<&[u8], &[u8]> = TableDefinition::new(handle.name());
                let table = read_txn.open_table(def)?;
                for entry in table.iter()? {
                    let (k, _v) = entry?;
                    keys.push((handle.name().to_string(), k.value().to_vec()));
                }
            }
        }
        let mut removed = 0u64;
        for (table, key) in keys {
            let usage = self.usage()?;
            let over_rows = policy.max_rows.is_some_and(|m| usage.row_count > m);
            let over_bytes = policy.max_bytes.is_some_and(|m| usage.byte_size > m);
            if !over_rows && !over_bytes {
                break;
            }
            if self.delete(&table, &key)? {
                removed += 1;
            }
        }
        let after = self.file_bytes()?;
        let reclaimed = before.saturating_sub(after);
        Ok(PruneReport { rows_removed: removed, bytes_reclaimed: reclaimed })
    }

    fn file_bytes(&self) -> Result<u64, KvError> {
        if !self.db_path.exists() {
            return Ok(0);
        }
        Ok(fs::metadata(&self.db_path)
            .map_err(|e| IoFault::Read { path: self.db_path.clone(), source: e })?
            .len())
    }
}
