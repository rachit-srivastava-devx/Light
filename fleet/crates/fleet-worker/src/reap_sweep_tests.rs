use super::*;
use std::process::{Command, Stdio};

fn git_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let run = |args: &[&str]| {
        let status = Command::new("git").arg("-C").arg(dir.path()).args(args).status().unwrap();
        assert!(status.success());
    };
    run(&["init", "-q"]);
    run(&["config", "user.email", "t@example.com"]);
    run(&["config", "user.name", "t"]);
    std::fs::write(dir.path().join("f.txt"), b"x").unwrap();
    run(&["add", "."]);
    run(&["commit", "-q", "-m", "init"]);
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
fn sweeps_a_real_dead_worktree_through_the_containment_guarded_remove() {
    let repo = git_repo();
    let owner = dead_pid();
    let worker = dead_pid();
    let name = format!("builder-{owner}-0");
    let worktree = fleet_merge::create(repo.path(), &name).expect("create");
    crate::reap::record_worker_pid(&worktree.path, worker);
    assert!(worktree.path.exists());

    let results = reap_dead_lanes(repo.path());
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, worktree.path);
    assert!(results[0].1.is_ok(), "{:?}", results[0].1);
    assert!(!worktree.path.exists(), "the swept worktree must actually be gone");
}

#[test]
fn leaves_a_live_owned_worktree_alone() {
    let repo = git_repo();
    // `unique_name` embeds THIS test process's own (very much alive) pid.
    let name = fleet_merge::unique_name("builder");
    let worktree = fleet_merge::create(repo.path(), &name).expect("create");
    crate::reap::record_worker_pid(&worktree.path, dead_pid());

    assert!(reap_dead_lanes(repo.path()).is_empty());
    assert!(worktree.path.exists(), "a lane with a live owner must not be touched");
    let _ = fleet_merge::remove(repo.path(), &worktree);
}
