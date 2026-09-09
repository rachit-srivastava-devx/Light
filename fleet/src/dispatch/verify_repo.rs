//! `ensure_repo` -- the S1 pt. 3 refusal: a nonexistent/unreadable `--repo` given to `fleet
//! gate`/`fleet oracle` must be a typed error and a non-zero exit, never a silent fallback to the
//! calling process's own cwd. Reuses `walk::ensure_repo_readable`, the same rule `graph`/`impact`
//! already enforce for their own `--repo`, so "refuse an unusable target" stays one rule.

use super::error::DispatchError;
use super::walk::ensure_repo_readable;
use std::path::{Path, PathBuf};

pub fn ensure_repo(repo: &str) -> Result<PathBuf, DispatchError> {
    let path = Path::new(repo).to_path_buf();
    ensure_repo_readable(&path)?;
    Ok(path)
}
