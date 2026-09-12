//! `open()`/`get()`/`put()`/`delete()`/`list_prefix()` via redb, one named table per caller.

use std::path::Path;

use redb::{Database, ReadableTable, TableDefinition};

use super::types::KvError;

pub struct KvStore {
    pub(crate) db: Database,
    pub(crate) db_path: std::path::PathBuf,
}

impl KvStore {
    pub fn open(path: &Path) -> Result<Self, KvError> {
        Ok(Self { db: Database::create(path)?, db_path: path.to_path_buf() })
    }

    pub fn get(&self, table: &str, key: &[u8]) -> Result<Option<Vec<u8>>, KvError> {
        let def: TableDefinition<&[u8], &[u8]> = TableDefinition::new(table);
        let read_txn = self.db.begin_read()?;
        let table = match read_txn.open_table(def) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        Ok(table.get(key)?.map(|v| v.value().to_vec()))
    }

    pub fn put(&self, table: &str, key: &[u8], value: &[u8]) -> Result<(), KvError> {
        let def: TableDefinition<&[u8], &[u8]> = TableDefinition::new(table);
        let write_txn = self.db.begin_write()?;
        {
            let mut t = write_txn.open_table(def)?;
            t.insert(key, value)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Returns whether a value was actually removed.
    pub fn delete(&self, table: &str, key: &[u8]) -> Result<bool, KvError> {
        let def: TableDefinition<&[u8], &[u8]> = TableDefinition::new(table);
        let write_txn = self.db.begin_write()?;
        let removed = {
            let mut t = write_txn.open_table(def)?;
            let prior = t.remove(key)?;
            prior.is_some()
        };
        write_txn.commit()?;
        Ok(removed)
    }

    #[allow(clippy::type_complexity)]
    pub fn list_prefix(&self, table: &str, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, KvError> {
        let def: TableDefinition<&[u8], &[u8]> = TableDefinition::new(table);
        let read_txn = self.db.begin_read()?;
        let t = match read_txn.open_table(def) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };
        let mut out = Vec::new();
        for entry in t.iter()? {
            let (k, v) = entry?;
            if k.value().starts_with(prefix) {
                out.push((k.value().to_vec(), v.value().to_vec()));
            }
        }
        Ok(out)
    }
}
