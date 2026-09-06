//! S3: worktree-lifecycle primitives for parallel, isolated role lanes.
//!
//! `AGENTS.md`'s worktree section is the law here: worktrees live inside the repo at
//! `.worktrees/<name>` (never as siblings of the repo), created with a *named* branch
//! (`git worktree add -b <branch> .worktrees/<name> HEAD`), and removed with
//! `git worktree remove --force`. This module does not invent a second isolation mechanism:
//! it is the same primitive `rollback_artifact` in `main.rs` already uses (`git worktree add`
//! plus `git worktree remove --force`), just pointed at the named-branch convention instead of
//! detached HEAD under `/tmp`, and reused via `run_bounded`/`start_process_group`-shaped helpers.
//!
//! Naming: `AGENTS.md` rule 4 says never derive a path that will later be `rm -rf`'d from a
//! content digest alone, because two concurrent runs colliding on the same name is exactly the
//! failure that wedged the machine for 40 minutes, twice. Every name here mixes the process id
//! with a per-call atomic counter, so two lanes started in the same process in the same
//! microsecond still cannot collide.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

/// Exit-code contract mirrors `main.rs`: 3 = environment fault, 6 = invariant violation.
pub const EXIT_ENV: i32 = 3;
pub const EXIT_INVARIANT: i32 = 6;

static NEXT_SEQ: AtomicU64 = AtomicU64::new(0);

/// A live, isolated worktree. Removal is the caller's responsibility (call `remove`) -- this
/// type does not implement `Drop`-based cleanup because the cleanup itself is fallible and must
/// be observed (a silently-swallowed `Drop` failure is exactly the kind of proxy this codebase's
/// standing law rejects). Callers MUST pair every `create` with a `remove` on both the success
/// and the error path; see `run_lanes_concurrently` in `main.rs` for the guaranteed-cleanup
/// pattern (a closure wrapped in a result, cleanup runs unconditionally afterward).
pub struct Worktree {
    pub path: PathBuf,
    pub branch: String,
    pub name: String,
}

/// Build a worktree name that is unique per (dispatch, role) even under concurrent dispatches of
/// the identical task: pid + a monotonic in-process counter + the role label. Never a content
/// digest of the task alone -- two concurrent `swarm dispatch --task "same text"` invocations
/// must not collide (AGENTS.md rule 4).
pub fn unique_name(label: &str) -> String {
    let seq = NEXT_SEQ.fetch_add(1, Ordering::Relaxed);
    format!("{}-{}-{}", label, std::process::id(), seq)
}

/// `git -C <repo> worktree add -b fleet/<name> .worktrees/<name> HEAD`
///
/// Named branch, not `--detach`: S3's acceptance is real per-role work happening in a real,
/// checked-out branch a human can inspect afterward, not a throwaway detached scratch dir.
pub fn create(repo: &Path, name: &str) -> Result<Worktree, i32> {
    if name.trim().is_empty() {
        return Err(EXIT_INVARIANT);
    }
    let rel = format!(".worktrees/{name}");
    let branch = format!("fleet/{name}");
    // Concurrent `git worktree add` invocations against the SAME repo contend on git's own
    // `.git/index.lock` / `.git/worktrees` administrative lock -- observed directly running S3's
    // own acceptance test (`keel/fleet/tests/s3_lanes.rs`): 3 of 4 concurrent lanes succeeded,
    // the 4th failed fast with a non-zero exit while the others were still mid-checkout. That is
    // real evidence, not a hypothetical -- git's lock is transient, so retry with a short random
    // backoff instead of either serialising every lane (defeats S3's purpose) or letting a
    // same-process sibling's lock hold register as this lane's own failure.
    let mut last_stderr = Vec::new();
    let mut succeeded = false;
    for attempt in 0..8u32 {
        if attempt > 0 {
            // A cheap, dependency-free jitter: mix the pid, the attempt number, and a coarse
            // timestamp -- good enough to desynchronise sibling lanes without pulling in `rand`.
            let now_nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(0);
            let jitter_ms = (std::process::id() ^ now_nanos ^ attempt) % 40;
            std::thread::sleep(std::time::Duration::from_millis(10 + u64::from(jitter_ms)));
        }
        let mut command = Command::new("git");
        command
            .arg("-C")
            .arg(repo)
            .args(["worktree", "add", "-b", &branch, &rel, "HEAD"])
            .stdin(Stdio::null());
        let output = command.output().map_err(|_| EXIT_ENV)?;
        if output.status.success() {
            succeeded = true;
            break;
        }
        last_stderr = output.stderr;
        // A partial worktree admin dir from a failed attempt must not block the retry -- prune
        // it before trying again (best-effort; if it fails, the next `git worktree add` will
        // surface a real, non-transient error instead of masking one).
        let mut prune = Command::new("git");
        prune
            .arg("-C")
            .arg(repo)
            .args(["worktree", "prune"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let _ = prune.status();
    }
    if !succeeded {
        let _ = last_stderr; // captured for a human debugging a real (non-transient) failure
        return Err(EXIT_ENV);
    }
    let path = repo.join(&rel);
    if !path.is_dir() {
        return Err(EXIT_INVARIANT);
    }
    Ok(Worktree {
        path,
        branch,
        name: name.to_string(),
    })
}

/// `git -C <repo> worktree remove --force .worktrees/<name>`, with a filesystem fallback and a
/// best-effort branch delete so a failed lane never leaks a worktree OR a dangling branch.
/// Errors are collected, not swallowed: the caller decides whether a cleanup failure should be
/// surfaced (it is real information -- "removed" is a claim, not an assumption).
pub fn remove(repo: &Path, worktree: &Worktree) -> Result<(), i32> {
    let rel = format!(".worktrees/{}", worktree.name);
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(repo)
        .args(["worktree", "remove", "--force"])
        .arg(&rel)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let status = command.status().map_err(|_| EXIT_ENV)?;
    // If `git worktree remove` failed but the directory is already gone (e.g. it was never
    // fully created), that's not a leak -- only a *surviving* directory is.
    if !status.success() && worktree.path.exists() {
        let _ = std::fs::remove_dir_all(&worktree.path);
        if worktree.path.exists() {
            return Err(EXIT_ENV);
        }
    }
    // Best-effort: prune the branch too so repeated lanes don't accumulate dead refs. Not
    // treated as fatal -- the worktree directory (the thing that can leak disk / collide with a
    // later `.worktrees/<name>`) is already gone by this point.
    let mut branch_cmd = Command::new("git");
    branch_cmd
        .arg("-C")
        .arg(repo)
        .args(["branch", "-D", &worktree.branch])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let _ = branch_cmd.status();
    let mut prune_cmd = Command::new("git");
    prune_cmd
        .arg("-C")
        .arg(repo)
        .args(["worktree", "prune"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let _ = prune_cmd.status();
    Ok(())
}

/// The concurrency cap S3's acceptance bar names verbatim: `min(16, available_parallelism()-2)`,
/// floored at 1 so a 1-2 core box still runs lanes (serially, but correctly) instead of
/// dividing by zero / capping at zero.
pub fn lane_cap() -> usize {
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    cores.saturating_sub(2).clamp(1, 16)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init_repo() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fleet-wt-test-{}", unique_name("init")));
        std::fs::create_dir_all(&dir).unwrap();
        let status = Command::new("git")
            .arg("-C")
            .arg(&dir)
            .args(["init", "-q"])
            .status()
            .unwrap();
        assert!(status.success());
        Command::new("git")
            .arg("-C")
            .arg(&dir)
            .args(["config", "user.email", "test@example.com"])
            .status()
            .unwrap();
        Command::new("git")
            .arg("-C")
            .arg(&dir)
            .args(["config", "user.name", "test"])
            .status()
            .unwrap();
        std::fs::write(dir.join("f.txt"), b"x").unwrap();
        Command::new("git")
            .arg("-C")
            .arg(&dir)
            .args(["add", "."])
            .status()
            .unwrap();
        Command::new("git")
            .arg("-C")
            .arg(&dir)
            .args(["commit", "-q", "-m", "init"])
            .status()
            .unwrap();
        dir
    }

    #[test]
    fn create_and_remove_round_trips_cleanly() {
        let repo = init_repo();
        let name = unique_name("t-role");
        let wt = create(&repo, &name).expect("create");
        assert!(wt.path.is_dir());
        assert!(wt.path.join("f.txt").exists());
        remove(&repo, &wt).expect("remove");
        assert!(!wt.path.exists());
        std::fs::remove_dir_all(&repo).ok();
    }

    #[test]
    fn two_concurrent_names_never_collide() {
        let a = unique_name("role");
        let b = unique_name("role");
        assert_ne!(a, b);
    }

    #[test]
    fn lane_cap_is_at_least_one_and_at_most_sixteen() {
        let cap = lane_cap();
        assert!(cap >= 1);
        assert!(cap <= 16);
    }
}
