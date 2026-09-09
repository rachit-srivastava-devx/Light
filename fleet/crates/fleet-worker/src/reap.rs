//! Stale-lane detection for a NEXT `fleet` invocation, after a prior one died (e.g. `kill -9`)
//! mid-run and never called `join`/`fleet_merge::remove`. A leaked `<repo>/.worktrees/<name>`
//! must be provably dead before anything treats it as reapable -- never guessed from mtime or
//! a directory simply being present. "Provably dead" means BOTH: the fleet process that
//! created the lane (pid embedded in the worktree name, `<label>-<pid>-<seq>`) and the worker
//! it spawned (recorded by `record_worker_pid` below) are gone. Missing either signal: left alone.

use nix::sys::signal::kill;
use nix::unistd::Pid;
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(test)]
#[path = "reap_tests.rs"]
mod tests;

const LANE_PID_FILE: &str = ".fleet-lane.pid";

/// Called by `spawn` right after `Command::spawn()` succeeds. Best-effort: a failed write just
/// means this lane cannot later be proven dead by `find_dead_lanes`, never a spawn failure.
pub fn record_worker_pid(worktree_path: &Path, worker_pid: u32) {
    let _ = fs::write(worktree_path.join(LANE_PID_FILE), worker_pid.to_string());
}

/// Removes the marker before `join()`'s honesty check -- fleet's own bookkeeping must never
/// count as worker-made change. Safe: the child has exited by this point already.
pub fn clear_worker_pid(worktree_path: &Path) {
    let _ = fs::remove_file(worktree_path.join(LANE_PID_FILE));
}

fn is_alive(pid: i32) -> bool {
    kill(Pid::from_raw(pid), None).is_ok()
}

/// Direct children of `<repo>/.worktrees` fleet can prove are dead by the rule above. The
/// caller (a future `fleet rollback`/startup sweep) is responsible for actually removing them,
/// through the existing containment-guarded path -- this module only proves, never deletes.
pub fn find_dead_lanes(repo: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(repo.join(".worktrees")) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir() && is_dead_lane(p))
        .collect()
}

fn is_dead_lane(path: &Path) -> bool {
    // A real `git worktree add`-created lane has a `.git` FILE (a pointer back to the main
    // repo's `.git/worktrees/<name>`), never a `.git` dir or nothing -- rules out an arbitrary
    // directory shaped to look reapable.
    if !path.join(".git").is_file() {
        return false;
    }
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    let Some(owner_pid) = owner_pid_from_name(name) else {
        return false;
    };
    let Ok(raw) = fs::read_to_string(path.join(LANE_PID_FILE)) else {
        return false;
    };
    let Ok(worker_pid) = raw.trim().parse::<i32>() else {
        return false;
    };
    !is_alive(owner_pid) && !is_alive(worker_pid)
}

/// The middle field of `fleet_merge::unique_name`'s `<label>-<pid>-<seq>`. A name that does not
/// fit this exact three-field shape is not a lane fleet recognises, so it is never touched.
fn owner_pid_from_name(name: &str) -> Option<i32> {
    let parts: Vec<&str> = name.rsplitn(3, '-').collect();
    if parts.len() != 3 {
        return None;
    }
    parts[1].parse().ok()
}
