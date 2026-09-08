//! `rows()` and the `read_rows` helper shared with `append`/`verify`.

use std::fs;
use std::path::Path;

use fleet_types::Receipt;

use super::types::LedgerError;
use super::Ledger;
use crate::io_fault::IoFault;
use crate::lock::FileLock;

impl Ledger {
    /// Every row in chain order. `allow_empty = false` treats "never initialized" as an
    /// invariant violation (`Err(LedgerError::Empty)`), not a valid zero state.
    pub fn rows(&self, allow_empty: bool) -> Result<Vec<Receipt>, LedgerError> {
        let _guard = FileLock::acquire(&self.paths.lock)?;
        let rows = read_rows(&self.paths.chain)?;
        if !allow_empty && rows.is_empty() {
            return Err(LedgerError::Empty);
        }
        Ok(rows)
    }
}

/// Reads and parses every JSONL line into a typed `Receipt`. A line that fails to parse (missing
/// field, bad hash shape, unknown event, ...) surfaces as `MalformedRow` at that line's index.
pub(super) fn read_rows(path: &Path) -> Result<Vec<Receipt>, LedgerError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(path)
        .map_err(|source| IoFault::Read { path: path.to_path_buf(), source })?;
    let mut rows = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let receipt: Receipt = serde_json::from_str(line).map_err(|e| LedgerError::MalformedRow {
            seq: idx as u64,
            reason: e.to_string(),
        })?;
        rows.push(receipt);
    }
    Ok(rows)
}
