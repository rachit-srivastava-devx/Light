//! `ensure_repo` refuses a directory that is not inside a git worktree with a clear,
//! `EnvFault`-typed error, and accepts one that is. Pins the friction: `fleet run --repo
//! <workspace of sibling checkouts>` used to sail past intake and fail deep in a per-gate git
//! command; now it errors up front with the "point --repo at the repo root" message.

use super::ensure_repo;
use crate::dispatch::error::DispatchError;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn a_directory_that_is_not_a_git_worktree_is_refused_with_env_fault_and_actionable_message() {
    let dir = tempdir().expect("tempdir");
    let path = dir.path().to_str().expect("utf-8 tempdir");
    let err = ensure_repo(path).expect_err("a non-git directory must not pass intake");
    let DispatchError::EnvFault(msg) = &err else {
        panic!("expected EnvFault, got {err:?}");
    };
    assert!(msg.contains("not a git repository"), "message must name the failure: {msg}");
    assert!(msg.contains("--repo"), "message must reference the flag: {msg}");
    assert_eq!(err.exit_code().as_i32(), 3, "EnvFault must map to exit 3");
}

#[test]
fn a_fresh_git_init_directory_passes_intake() {
    let dir = tempdir().expect("tempdir");
    let status = Command::new("git")
        .arg("-C")
        .arg(dir.path())
        .arg("init")
        .arg("--quiet")
        .status()
        .expect("git init");
    assert!(status.success(), "git init must succeed for the test to be meaningful");
    let path = dir.path().to_str().expect("utf-8 tempdir");
    ensure_repo(path).expect("a fresh git-init'd directory must pass intake");
}
