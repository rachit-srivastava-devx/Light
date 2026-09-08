//! Writes validated targets to disk. Only called after every target in the batch has already
//! been resolved and containment-checked (`guard::resolve_target`) for the WHOLE batch -- so by
//! the time this runs there is nothing left to reject; a bad target already stopped `apply`
//! before any write. Directories are created for every target before any file is written, so a
//! `create_dir_all` failure on file N never leaves files 1..N-1 written and N missing.

use super::error::ApplyError;
use std::path::PathBuf;

pub fn write_all(files: &[(PathBuf, String)]) -> Result<Vec<PathBuf>, ApplyError> {
    for (path, _) in files {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| ApplyError::Write { path: path.clone(), source })?;
        }
    }
    for (path, content) in files {
        std::fs::write(path, content).map_err(|source| ApplyError::Write { path: path.clone(), source })?;
    }
    Ok(files.iter().map(|(p, _)| p.clone()).collect())
}
