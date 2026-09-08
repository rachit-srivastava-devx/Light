//! `lane_changed`: did the worktree end up different from `base_commit` -- the `HEAD` it had
//! the moment `fleet_merge::create` made it -- by EITHER measure: a dirty tree (uncommitted
//! edits/untracked files) or a moved `HEAD` (the worker committed). Either one alone is
//! "changed". A worker that commits and then `git reset --hard`s back onto `base_commit` nets
//! to the same tree it started with, so that case must still read as unchanged -- which falls
//! out for free from comparing final `HEAD` to `base_commit` by sha equality, no separate case
//! needed. A worktree left on a detached `HEAD` also falls out for free: `rev-parse HEAD`
//! resolves a detached `HEAD` to its commit sha exactly like an attached one.

use super::error::ChangeDetectError;
use std::path::Path;
use std::process::{Command, Stdio};

pub(super) fn lane_changed(worktree: &Path, base_commit: &str) -> Result<bool, ChangeDetectError> {
    if dirty_file_count(worktree)? > 0 {
        return Ok(true);
    }
    Ok(current_head(worktree)? != base_commit)
}

fn dirty_file_count(worktree: &Path) -> Result<usize, ChangeDetectError> {
    let output = git(worktree, &["status", "--porcelain", "--untracked-files=all"])
        .map_err(|detail| ChangeDetectError::StatusUnavailable { path: worktree.to_path_buf(), detail })?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).into_owned();
        return Err(ChangeDetectError::StatusUnavailable { path: worktree.to_path_buf(), detail });
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.lines().filter(|l| !l.trim().is_empty()).count())
}

fn current_head(worktree: &Path) -> Result<String, ChangeDetectError> {
    let output = git(worktree, &["rev-parse", "HEAD"])
        .map_err(|detail| ChangeDetectError::HeadUnreadable { path: worktree.to_path_buf(), detail })?;
    let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if !output.status.success() || sha.is_empty() {
        let detail = String::from_utf8_lossy(&output.stderr).into_owned();
        return Err(ChangeDetectError::HeadUnreadable { path: worktree.to_path_buf(), detail });
    }
    Ok(sha)
}

/// Test-only seam: when set, execs this program name instead of `git`. Lets an integration
/// test force a git-invocation failure here specifically (the S2/rule-7 repro) without breaking
/// `PATH` for the whole `join()` call, which would also break `fleet_merge::remove`'s own,
/// unrelated git calls a few lines later in the same call. Production code never sets this.
const GIT_OVERRIDE_ENV: &str = "FLEET_WORKER_TEST_CHANGE_DETECT_GIT";

fn git(worktree: &Path, args: &[&str]) -> Result<std::process::Output, String> {
    let program = std::env::var(GIT_OVERRIDE_ENV).unwrap_or_else(|_| "git".to_string());
    Command::new(program).arg("-C").arg(worktree).args(args).stdin(Stdio::null()).output().map_err(|e| e.to_string())
}
