//! `Ledger::usage()` and `Ledger::prune()` -- reporting and refusal.

use std::fs;

use super::types::LedgerError;
use super::Ledger;
use crate::io_fault::IoFault;
use crate::retention::{PruneReport, RetentionPolicy, UsageReport};

impl Ledger {
    /// Report the current on-disk size of the ledger chain file. `row_count` is the number
    /// of JSONL lines; `byte_size` is the file length in bytes.
    pub fn usage(&self) -> Result<UsageReport, LedgerError> {
        // `rows()` acquires `self.paths.lock` itself -- taking it here too would deadlock (the
        // advisory flock is per-fd, not reentrant within a process).
        let rows = self.rows(true)?;
        let byte_size = if self.paths.chain.exists() {
            fs::metadata(&self.paths.chain)
                .map_err(|source| IoFault::Read { path: self.paths.chain.clone(), source })?
                .len()
        } else {
            0
        };
        Ok(UsageReport { row_count: rows.len() as u64, byte_size })
    }

    /// Always refused: the ledger is append-only and hash-chained. Deleting any row would break
    /// the blake3 chain's verifiability. The caller must use archival or segment rotation instead.
    pub fn prune(&self, _policy: &RetentionPolicy) -> Result<PruneReport, LedgerError> {
        Err(LedgerError::LedgerRefused)
    }
}
