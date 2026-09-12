//! Shared filesystem-level failure, common to every store in this crate.

use std::path::PathBuf;

/// A failure at the OS boundary (open/lock/read/write) -- never a domain-logic failure, which
/// each store's own error enum carries instead.
#[derive(Debug, thiserror::Error)]
pub enum IoFault {
    #[error("could not open {path}: {source}")]
    Open { path: PathBuf, source: std::io::Error },
    #[error("could not acquire exclusive lock on {path}: {source}")]
    Lock { path: PathBuf, source: std::io::Error },
    #[error("could not read {path}: {source}")]
    Read { path: PathBuf, source: std::io::Error },
    #[error("could not write {path}: {source}")]
    Write { path: PathBuf, source: std::io::Error },
}
