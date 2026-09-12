//! The append-only, blake3 hash-chained ledger -- `main.rs:4302-4605`.

mod append;
mod canon;
mod clock;
mod read;
mod tail;
mod types;
mod usage;
mod verify;

pub use types::{LedgerError, LedgerPaths, VerifiedChain};

/// A handle to one ledger. Opening does no IO -- IO happens per call, under the file lock, so a
/// `Ledger` value can be constructed freely and shared by reference across threads/tasks.
pub struct Ledger {
    paths: LedgerPaths,
}

impl Ledger {
    /// Construct a handle. Does not touch the filesystem; `chain`/`lock` need not exist yet.
    pub fn open(paths: LedgerPaths) -> Self {
        Self { paths }
    }
}
