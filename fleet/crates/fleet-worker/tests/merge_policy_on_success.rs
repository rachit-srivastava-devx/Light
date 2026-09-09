//! `MergePolicy::OnSuccess` regression pins (real temp git repo, this crate's convention): a
//! `Done` lane with a real change actually merges and the worktree is gone; a `Refused` lane
//! never merges but is still cleaned up -- a merge refusal must not leak a worktree.
mod common;

use common::{fixture_exe, head, init_repo, lock_env};
use fleet_types::TaskId;
use fleet_worker::{join, spawn, CliAdapter, LaneOutcome, MergePolicy, Role, SpawnRequest};
use std::process::Command;
use std::time::Duration;

fn spawn_fixture(repo: &std::path::Path, task: &str) -> fleet_worker::LaneHandle {
    std::env::set_var("FLEET_WORKER_TEST_CHILD_EXE", fixture_exe());
    let request = SpawnRequest {
        repo: repo.to_path_buf(),
        role: Role::Builder,
        task_id: TaskId::parse("t1").unwrap(),
        adapter: CliAdapter::Freelane,
        requested_model: None,
        task: task.to_string(),
        deadline: Duration::from_secs(10),
    };
    let handle = spawn(request).expect("spawn");
    std::env::remove_var("FLEET_WORKER_TEST_CHILD_EXE");
    handle
}

#[test]
fn on_success_with_real_change_merges_head_and_removes_the_worktree() {
    let _guard = lock_env();
    let repo = init_repo();
    let before = head(&repo);
    let handle = spawn_fixture(&repo, "done");
    let worktree_path = handle.worktree_path.clone();
    let (outcome, merge) = join(handle, MergePolicy::OnSuccess).expect("join");
    assert!(matches!(outcome, LaneOutcome::Done { .. }), "{outcome:?}");
    let m = merge.expect("Done + OnSuccess must produce a MergeOutcome");
    assert_eq!(m.changed_files, 1);
    let after = head(&repo);
    assert_ne!(after, before, "HEAD must move after a real merge");
    assert_eq!(after, m.after);

    let show =
        Command::new("git").arg("-C").arg(&repo).args(["show", "--name-only", &after]).output().unwrap();
    let text = String::from_utf8_lossy(&show.stdout);
    assert!(text.contains("fixture-change.txt"), "merge commit must carry the worker's edit: {text}");

    let list = Command::new("git").arg("-C").arg(&repo).args(["worktree", "list"]).output().unwrap();
    let listing = String::from_utf8_lossy(&list.stdout);
    let path_str = worktree_path.to_string_lossy();
    assert!(!listing.contains(path_str.as_ref()), "lane worktree must be gone: {listing}");
    std::fs::remove_dir_all(&repo).ok();
}

#[test]
fn on_success_with_refused_outcome_never_merges_but_still_tears_down() {
    let _guard = lock_env();
    let repo = init_repo();
    let before = head(&repo);
    let handle = spawn_fixture(&repo, "refuse");
    let worktree_path = handle.worktree_path.clone();
    let (outcome, merge) = join(handle, MergePolicy::OnSuccess).expect("join");
    assert!(matches!(outcome, LaneOutcome::Refused { .. }), "{outcome:?}");
    assert!(merge.is_none(), "a Refused outcome must never be merged");
    assert_eq!(head(&repo), before, "a Refused outcome must never move HEAD");
    assert!(!worktree_path.exists(), "the worktree must still be torn down on a merge no-op");
    std::fs::remove_dir_all(&repo).ok();
}
