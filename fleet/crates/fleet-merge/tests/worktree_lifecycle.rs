//! Worktree create/remove integration tests against real `tempfile::TempDir` git repos.

mod common;

use common::init_repo;
use fleet_merge::{create, remove, unique_name, WorktreeError};

#[test]
fn create_and_remove_round_trips_cleanly() {
    let (_guard, repo) = init_repo();
    let name = unique_name("t-role");
    let wt = create(&repo, &name).expect("create");
    assert!(wt.path.is_dir());
    assert!(wt.path.join("f.txt").exists());
    remove(&repo, &wt).expect("remove");
    assert!(!wt.path.exists());
}

#[test]
fn create_rejects_empty_name_without_touching_git() {
    let (_guard, repo) = init_repo();
    for bad in ["", "   "] {
        let err = create(&repo, bad).expect_err("empty name must refuse");
        assert!(matches!(err, WorktreeError::EmptyName));
    }
    assert!(!repo.join(".worktrees").exists());
}

#[test]
fn create_twice_with_same_name_fails_on_retry_exhaustion() {
    let (_guard, repo) = init_repo();
    let name = unique_name("dup");
    let first = create(&repo, &name).expect("first create succeeds");
    let second = create(&repo, &name);
    assert!(matches!(second, Err(WorktreeError::CreateFailed { .. })));
    remove(&repo, &first).expect("cleanup");
}

#[test]
fn remove_is_idempotent_on_an_already_removed_worktree() {
    let (_guard, repo) = init_repo();
    let name = unique_name("idem");
    let wt = create(&repo, &name).expect("create");
    remove(&repo, &wt).expect("first remove");
    remove(&repo, &wt).expect("second remove is a no-op success");
}

#[test]
fn two_concurrent_names_never_collide() {
    let a = unique_name("role");
    let b = unique_name("role");
    assert_ne!(a, b);
}

#[cfg(unix)]
#[test]
fn remove_reports_leak_when_directory_survives_forced_remove() {
    use std::os::unix::fs::PermissionsExt;
    let (_guard, repo) = init_repo();
    let name = unique_name("leak");
    let wt = create(&repo, &name).expect("create");
    // Detach from git's worktree admin list so `git worktree remove` itself fails, then make the
    // directory unwritable so the filesystem fallback (`remove_dir_all`) also cannot clear it.
    common::run(&repo, &["worktree", "prune"]);
    std::fs::remove_dir_all(repo.join(".git/worktrees")).ok();
    let mut perms = std::fs::metadata(&wt.path).unwrap().permissions();
    perms.set_mode(0o555);
    std::fs::set_permissions(&wt.path, perms).unwrap();
    let err = remove(&repo, &wt).expect_err("leaked directory must be reported");
    assert!(matches!(err, WorktreeError::RemoveLeaked { .. }));
    let mut perms = std::fs::metadata(&wt.path).unwrap().permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&wt.path, perms).unwrap();
    std::fs::remove_dir_all(&wt.path).ok();
}
