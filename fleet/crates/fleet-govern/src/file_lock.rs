//! Shared OS advisory-lock helper for `FileMeterStore`/`FileCooldownStore`: open (creating if
//! needed) a sibling `.lock` file, acquire an exclusive `fs4` lock, run `body`, then unlock.
//! Split out so both file-backed stores share one lock-acquisition path.

use std::fs::OpenOptions;
use std::path::{Path, PathBuf};

use fs4::fs_std::FileExt;

use crate::types::MeterIoError;

pub(crate) fn lock_path_for(data_path: &Path) -> PathBuf {
    let mut p = data_path.as_os_str().to_owned();
    p.push(".lock");
    PathBuf::from(p)
}

pub(crate) fn with_exclusive_lock<T>(
    data_path: &Path,
    body: impl FnOnce() -> Result<T, MeterIoError>,
) -> Result<T, MeterIoError> {
    let lock_file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(lock_path_for(data_path))
        .map_err(|e| MeterIoError(format!("open lock: {e}")))?;
    lock_file.lock_exclusive().map_err(|e| MeterIoError(format!("lock: {e}")))?;
    let result = body();
    let _ = lock_file.unlock();
    result
}
