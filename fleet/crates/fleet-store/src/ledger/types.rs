//! `LedgerPaths`, `LedgerError`, `VerifiedChain`.

use std::path::PathBuf;

use crate::io_fault::IoFault;

/// The two files the ledger needs, supplied by the caller -- this crate derives neither from
/// `$HOME`, a state-dir helper, nor any other ambient source.
pub struct LedgerPaths {
    pub chain: PathBuf,
    pub lock: PathBuf,
}

/// Why an append or a verify failed. Every variant names the offending row's sequence number.
#[derive(Debug, thiserror::Error)]
pub enum LedgerError {
    #[error(transparent)]
    Io(#[from] IoFault),
    #[error("row {seq} is not a well-formed receipt: {reason}")]
    MalformedRow { seq: u64, reason: String },
    #[error("row {seq} carries seq={found} but its chain position is {expected}")]
    SeqMismatch { seq: u64, found: u64, expected: u64 },
    #[error("row {seq} prev_hash does not match the prior row's hash -- the chain link is broken")]
    BrokenLink { seq: u64 },
    #[error("row {seq} reuses a prev_hash another row already claimed -- the chain forks")]
    Fork { seq: u64 },
    #[error("row {seq} hash does not match its recomputed content hash -- tampered after write")]
    Tampered { seq: u64 },
    #[error("the ledger's tail row is corrupt: {reason} -- refusing to append onto it")]
    CorruptTail { reason: String },
    #[error("the ledger is empty")]
    Empty,
    #[error("ledger pruning is refused: the chain is append-only and hash-chained; deleting rows would break verifiability")]
    LedgerRefused,
}

/// The outcome of a full chain walk. `checked == total` always holds when `Ok` is returned.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedChain {
    pub checked: u64,
    pub total: u64,
}
