//! Pins D2 (fleet-cli E2E findings): `remove` must not report success for a worktree that has
//! nothing to remove -- neither one that was already removed nor one that never existed. Split
//! out of `worktree_lifecycle.rs` to keep that file under the line budget.

mod common;

use common::init_repo;
use fleet_merge::{create, remove, unique_name, Worktree, WorktreeError};

#[test]
fn remove_reports_not_found_on_an_already_removed_worktree() {
    let (_guard, repo) = init_repo();
    let name = unique_name("idem");
    let wt = create(&repo, &name).expect("create");
    remove(&repo, &wt).expect("first remove");
    // The directory is already gone: a second `remove` must not report a made-up success --
    // it never had anything to remove this time. Previously `fleet rollback` on a path that
    // was never (or is no longer) there printed `ok: worktree removed`.
    let err = remove(&repo, &wt).expect_err("second remove has nothing to remove");
    assert!(matches!(err, WorktreeError::NotFound { .. }));
}

#[test]
fn remove_reports_not_found_for_a_path_that_never_existed() {
    let (_guard, repo) = init_repo();
    let wt = Worktree {
        path: repo.join(".worktrees/never-existed-xyz"),
        branch: "fleet/never-existed-xyz".to_string(),
        name: "never-existed-xyz".to_string(),
    };
    let err = remove(&repo, &wt).expect_err("path never existed");
    assert!(matches!(err, WorktreeError::NotFound { .. }));
}
