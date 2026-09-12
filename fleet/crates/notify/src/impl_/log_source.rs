//! The injected read boundary over the ledger. This crate never opens `ledger/chain.jsonl`
//! itself -- every fact about "what's new" arrives through `LogSource`, implemented by
//! `fleet-store` in production and by a test fake in this crate's own suite.

use fleet_types::Receipt;

/// Deliberately `&self`, not `&mut self`: a real store answers "give me every receipt after
/// seq N" as a stateless query, which is what lets every sink's worker poll independently
/// without sharing a mutable cursor.
pub trait LogSource: Send + Sync {
    /// Every receipt with `seq > after` (or every receipt, if `after` is `None`), in ascending
    /// seq order. Must be pure w.r.t. its own state: two calls with the same `after` against an
    /// unchanged log return the same receipts.
    fn poll_since(&self, after: Option<u64>) -> Result<Vec<Receipt>, LogSourceError>;
}

/// Why a `LogSource::poll_since` call failed.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum LogSourceError {
    /// The store is temporarily unreachable (connection refused, file locked) -- retry later.
    #[error("log source unavailable: {0}")]
    Unavailable(String),
    /// The source returned a receipt whose `seq` did not strictly increase, or was `<= after`
    /// -- an invariant violation in the store, never expected in normal operation.
    #[error("log source returned seq {got} out of order (expected > {expected})")]
    OutOfOrder { expected: u64, got: u64 },
}
