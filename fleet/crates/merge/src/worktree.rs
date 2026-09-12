//! Worktree lifecycle -- verbatim port of `worktree.rs:27-164`, minus `lane_cap` (scheduling).

use crate::error::WorktreeError;
use crate::git_exec::{jittered_sleep, run_git, run_git_quiet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_SEQ: AtomicU64 = AtomicU64::new(0);

/// A live, isolated worktree. Removal is the caller's job (`remove`) -- no `Drop` cleanup, since a
/// swallowed `Drop` failure is the proxy this codebase rejects.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Worktree {
    pub path: PathBuf,
    pub branch: String,
    pub name: String,
}

/// Unique per call under concurrent callers: pid + a monotonic counter + the label. Never a
/// content digest of the task alone. Verbatim port of `worktree.rs:39-46`.
pub fn unique_name(label: &str) -> String {
    let seq = NEXT_SEQ.fetch_add(1, Ordering::Relaxed);
    format!("{}-{}-{}", label, std::process::id(), seq)
}

/// `git worktree add -b fleet/<name> .worktrees/<name> HEAD`, 8 jittered retries against git's transient `.git/index.lock` (port of `worktree.rs:52-116`).
pub fn create(repo: &Path, name: &str) -> Result<Worktree, WorktreeError> {
    if name.trim().is_empty() {
        return Err(WorktreeError::EmptyName);
    }
    let rel = format!(".worktrees/{name}");
    let branch = format!("fleet/{name}");
    let mut last_stderr = Vec::new();
    let mut succeeded = false;
    for attempt in 0..8u32 {
        if attempt > 0 {
            jittered_sleep(attempt);
        }
        let output = run_git(repo, &["worktree", "add", "-b", &branch, &rel, "HEAD"])
            .map_err(WorktreeError::Spawn)?;
        if output.status.success() {
            succeeded = true;
            break;
        }
        last_stderr = output.stderr;
        run_git_quiet(repo, &["worktree", "prune"]);
    }
    if !succeeded {
        let stderr = String::from_utf8_lossy(&last_stderr).into_owned();
        return Err(WorktreeError::CreateFailed { attempts: 8, stderr });
    }
    let path = repo.join(&rel);
    if !path.is_dir() {
        return Err(WorktreeError::MissingAfterCreate { path });
    }
    Ok(Worktree { path, branch, name: name.to_string() })
}

/// `git worktree remove --force .worktrees/<name>`, fs fallback + best-effort branch delete. A
/// path that never was a worktree is a typed `NotFound`; one outside `<repo>/.worktrees/` is a
/// refusal, because the fs fallback below is a RECURSIVE DELETE (see `worktree_guard`).
pub fn remove(repo: &Path, worktree: &Worktree) -> Result<(), WorktreeError> {
    if !worktree.path.exists() {
        return Err(WorktreeError::NotFound { path: worktree.path.clone() });
    }
    let owned = crate::worktree_guard::ensure_owned_worktree(repo, &worktree.path)?;
    let rel = format!(".worktrees/{}", worktree.name);
    let output =
        run_git(repo, &["worktree", "remove", "--force", &rel]).map_err(WorktreeError::Spawn)?;
    if !output.status.success() && worktree.path.exists() {
        let fail = |e: std::io::Error| WorktreeError::RemoveFailed { path: owned.clone(), source_msg: e.to_string() };
        std::fs::remove_dir_all(&owned).map_err(fail)?;
        if worktree.path.exists() {
            return Err(WorktreeError::RemoveLeaked { path: worktree.path.clone() });
        }
    }
    run_git_quiet(repo, &["branch", "-D", &worktree.branch]);
    run_git_quiet(repo, &["worktree", "prune"]);
    Ok(())
}
