use super::*;
use std::process::{Command, Stdio};

fn repo() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

/// Mimics the one detail `is_dead_lane` checks before anything else: a real linked worktree's
/// `.git` is a FILE (a pointer back to the main repo), never a directory or absent.
fn lane_dir(repo: &Path, name: &str) -> PathBuf {
    let dir = repo.join(".worktrees").join(name);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(".git"), "gitdir: ../../.git/worktrees/x\n").unwrap();
    dir
}

/// A short-lived child, waited on, so its pid is guaranteed no longer running.
fn dead_pid() -> u32 {
    let mut child = Command::new("true").stdout(Stdio::null()).spawn().unwrap();
    let pid = child.id();
    child.wait().unwrap();
    pid
}

#[test]
fn reaps_a_lane_whose_owner_and_worker_are_both_gone() {
    let repo = repo();
    let dead = dead_pid();
    let dir = lane_dir(repo.path(), &format!("builder-{dead}-0"));
    record_worker_pid(&dir, dead);
    assert_eq!(find_dead_lanes(repo.path()), vec![dir]);
}

#[test]
fn leaves_a_lane_alone_when_the_worker_pid_is_still_alive() {
    let repo = repo();
    let dead = dead_pid();
    let dir = lane_dir(repo.path(), &format!("builder-{dead}-0"));
    record_worker_pid(&dir, std::process::id());
    assert!(find_dead_lanes(repo.path()).is_empty());
}

#[test]
fn leaves_a_lane_alone_with_no_recorded_worker_pid() {
    let repo = repo();
    let dead = dead_pid();
    lane_dir(repo.path(), &format!("builder-{dead}-0"));
    assert!(find_dead_lanes(repo.path()).is_empty());
}

#[test]
fn leaves_a_lane_alone_whose_name_does_not_fit_the_shape() {
    let repo = repo();
    let dead = dead_pid();
    let dir = lane_dir(repo.path(), "not-a-lane-name");
    record_worker_pid(&dir, dead);
    assert!(find_dead_lanes(repo.path()).is_empty());
}

#[test]
fn leaves_alone_a_directory_that_is_not_actually_a_git_worktree() {
    let repo = repo();
    let dead = dead_pid();
    let dir = repo.path().join(".worktrees").join(format!("builder-{dead}-0"));
    fs::create_dir_all(&dir).unwrap();
    record_worker_pid(&dir, dead);
    assert!(find_dead_lanes(repo.path()).is_empty());
}
