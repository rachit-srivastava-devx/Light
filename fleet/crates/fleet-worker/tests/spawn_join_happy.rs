//! NOTE: this uses the "done" fixture scenario, which writes a real file
//! (`fixture-change.txt`) before returning `Done` -- it exercises the legitimate-change path
//! through the real `spawn()`/`join()` lifecycle, not the no-op case. See
//! `spawn_join_true_no_op_healthy_repo.rs` for a true no-op through the same real lifecycle.
mod common;

use common::{fixture_exe, init_repo, lock_env};
use fleet_types::TaskId;
use fleet_worker::{join, spawn, CliAdapter, LaneOutcome, MergePolicy, Role, SpawnRequest};
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
fn spawn_join_stub_agent_round_trips_cleanly() {
    let _guard = lock_env();
    let repo = init_repo();
    let handle = spawn_fixture(&repo, "done");
    let worktree_path = handle.worktree_path.clone();
    assert!(worktree_path.is_dir());
    let (outcome, _merge) = join(handle, MergePolicy::Never).expect("join");
    match outcome {
        LaneOutcome::Done { resolved_model, tokens, body } => {
            assert_eq!(resolved_model.as_deref(), Some("stub-1"));
            assert_eq!(tokens.map(|t| t.get()), Some(42));
            assert_eq!(body["ok"], true);
        }
        other => panic!("expected Done, got {other:?}"),
    }
    assert!(!worktree_path.exists(), "worktree must be removed after join");
    std::fs::remove_dir_all(&repo).ok();
}
