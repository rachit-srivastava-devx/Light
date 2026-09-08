//! Retention policy, prune/usage reports, and the store-agnostic error type.

/// Per-store retention limits. A `None` field means "no limit" for that dimension.
#[derive(Clone, Debug, Default)]
pub struct RetentionPolicy {
    /// Maximum age in seconds. Entries older than `now - max_age_secs` are eligible for pruning.
    pub max_age_secs: Option<u64>,
    /// Maximum number of rows/entries. If the store exceeds this, oldest entries are removed.
    pub max_rows: Option<u64>,
    /// Maximum byte size of the on-disk store. If exceeded, oldest entries are removed.
    pub max_bytes: Option<u64>,
}

/// What was actually removed by a prune call. Never a silent bool.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PruneReport {
    pub rows_removed: u64,
    pub bytes_reclaimed: u64,
}

/// Current on-disk size of a store, measured when the usage call is made.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UsageReport {
    pub row_count: u64,
    pub byte_size: u64,
}

/// Why a retention operation failed. Each variant names the store it applies to.
#[derive(Debug, thiserror::Error)]
pub enum RetentionError {
    #[error("ledger pruning is refused: the chain is append-only and hash-chained; deleting rows would break verifiability")]
    LedgerRefused,
    #[error("graph store: {0}")]
    Graph(#[from] crate::graph::GraphError),
    #[error("memory store: {0}")]
    Memory(#[from] crate::memory::MemoryError),
    #[error("kv store: {0}")]
    Kv(#[from] crate::kv::KvError),
    #[error("ledger: {0}")]
    Ledger(#[from] crate::ledger::LedgerError),
}
