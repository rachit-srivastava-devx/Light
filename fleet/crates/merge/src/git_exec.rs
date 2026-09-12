//! Shared subprocess helper: every `git` invocation in this crate goes through `run_git` so the
//! `Command::new("git")` boilerplate (and the "could not spawn git" mapping) lives in one place.
//! This is a *named* IO boundary, not an injected trait -- see the crate-level doc comment for
//! why: the invariants this crate protects are properties of real git output, so a mock git
//! would let a bug in the actual invocation slip through untested.

use std::path::Path;
use std::process::{Command, Output, Stdio};

/// Run `git -C <repo> <args>`, capturing stdout/stderr, never inheriting stdin. Returns the raw
/// `Output` on any exit code (including non-zero) -- callers inspect `.status.success()`
/// themselves; only a failure to *spawn* the process is folded into `Err`.
pub fn run_git(repo: &Path, args: &[&str]) -> Result<Output, String> {
    let mut command = Command::new("git");
    command.arg("-C").arg(repo).args(args).stdin(Stdio::null());
    command.output().map_err(|e| e.to_string())
}

/// Run `git -C <repo> <args>` for its exit status only, discarding stdout/stderr. Used for
/// best-effort cleanup steps (branch delete, worktree prune) whose failure is never surfaced.
pub fn run_git_quiet(repo: &Path, args: &[&str]) {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(repo)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let _ = command.status();
}

/// Count non-empty lines of git's line-per-path stdout (`diff --name-only`, `diff --cached
/// --name-only`) -- shared by `merge.rs`'s staged/changed-file counts.
pub fn count_lines(stdout: &[u8]) -> usize {
    String::from_utf8_lossy(stdout)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count()
}

/// Dependency-free retry jitter (pid/attempt/timestamp mix), accepted `SystemTime::now()`
/// exception (§4 of the blueprint): never compared against a caller-visible value or asserted
/// on in tests -- only the retry count and final success/failure are test-observable.
pub fn jittered_sleep(attempt: u32) {
    let now_nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let jitter_ms = (std::process::id() ^ now_nanos ^ attempt) % 40;
    std::thread::sleep(std::time::Duration::from_millis(10 + u64::from(jitter_ms)));
}
