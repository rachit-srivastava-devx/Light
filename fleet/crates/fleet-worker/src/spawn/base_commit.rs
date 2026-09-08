//! `read_head`: `git rev-parse HEAD` inside a just-created worktree -- the fixed point `join`'s
//! honesty check (`change_detect`) later compares the finished lane's `HEAD` against.

use crate::request::SpawnError;
use std::path::Path;
use std::process::{Command, Stdio};

pub(super) fn read_head(worktree: &Path) -> Result<String, SpawnError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(worktree)
        .args(["rev-parse", "HEAD"])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| SpawnError::BaseCommitUnreadable(e.to_string()))?;
    let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !output.status.success() || sha.is_empty() {
        let detail = String::from_utf8_lossy(&output.stderr).into_owned();
        return Err(SpawnError::BaseCommitUnreadable(detail));
    }
    Ok(sha)
}
