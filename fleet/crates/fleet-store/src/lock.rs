//! Advisory exclusive file lock, ported verbatim from `main.rs:4310-4328`'s `FileLock`.

use std::fs::{File, OpenOptions};
use std::path::Path;

use fs2::FileExt;

use crate::io_fault::IoFault;

/// Holds an exclusive advisory lock on a file for the lifetime of this value; released on drop.
pub(crate) struct FileLock(File);

impl FileLock {
    pub(crate) fn acquire(path: &Path) -> Result<Self, IoFault> {
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)
            .map_err(|source| IoFault::Open { path: path.to_path_buf(), source })?;
        file.lock_exclusive()
            .map_err(|source| IoFault::Lock { path: path.to_path_buf(), source })?;
        Ok(Self(file))
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.0);
    }
}
