//! `verify()`: the chain walk + per-row checks, first-failure-wins.

use std::collections::HashSet;

use fleet_types::{PrevHash, Receipt};

use super::canon::recompute_hash;
use super::types::{LedgerError, VerifiedChain};
use super::Ledger;

impl Ledger {
    /// Walk the full chain and verify sequence continuity, prev-hash linkage, fork-freedom, and
    /// content-hash match, in that order, per row. Returns the first failure found -- never
    /// aggregates multiple corruptions into one report.
    pub fn verify(&self) -> Result<VerifiedChain, LedgerError> {
        let rows = self.rows(true)?;
        if rows.is_empty() {
            return Err(LedgerError::Empty);
        }
        verify_rows(&rows)?;
        let total = rows.len() as u64;
        Ok(VerifiedChain { checked: total, total })
    }
}

pub(super) fn verify_rows(rows: &[Receipt]) -> Result<(), LedgerError> {
    let mut previous = PrevHash::Genesis;
    let mut seen_prev: HashSet<String> = HashSet::new();
    for (i, receipt) in rows.iter().enumerate() {
        let expected_seq = i as u64;
        if !receipt.body.is_object() {
            return Err(LedgerError::MalformedRow {
                seq: expected_seq,
                reason: "body must be a JSON object".to_string(),
            });
        }
        if receipt.seq != expected_seq {
            return Err(LedgerError::SeqMismatch {
                seq: expected_seq,
                found: receipt.seq,
                expected: expected_seq,
            });
        }
        if receipt.prev_hash.as_str() != previous.as_str() {
            return Err(LedgerError::BrokenLink { seq: expected_seq });
        }
        if !seen_prev.insert(receipt.prev_hash.as_str().to_string()) {
            return Err(LedgerError::Fork { seq: expected_seq });
        }
        let expected_hash = recompute_hash(receipt);
        if receipt.hash.as_str() != expected_hash {
            return Err(LedgerError::Tampered { seq: expected_seq });
        }
        previous = PrevHash::Hash(receipt.hash.clone());
    }
    Ok(())
}
