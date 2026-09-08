//! S1 end-to-end: a fixture agent that performs real work AND commits it inside the worktree
//! must still come back as `Done`, not a false `Refused` -- `git status` alone is clean for
//! committed work, so `join` must fall back to comparing `HEAD` against the worktree's base
//! commit (see `crates/fleet-worker/src/spawn/change_detect`).

mod common;

use common::{fixture_exe, init_repo, lock_env};
use fleet_types::TaskId;
use fleet_worker::{join, spawn, CliAdapter, LaneOutcome, Role, SpawnRequest};
use std::time::Duration;

#[test]
fn a_lane_that_commits_its_work_is_still_reported_as_done() {
    let _guard = lock_env();
    let repo = init_repo();
    std::env::set_var("FLEET_WORKER_TEST_CHILD_EXE", fixture_exe());
    let request = SpawnRequest {
        repo: repo.clone(),
        role: Role::Builder,
        task_id: TaskId::parse("t1").unwrap(),
        adapter: CliAdapter::Freelane,
        requested_model: None,
        task: "done_committed".to_string(),
        deadline: Duration::from_secs(10),
    };
    let handle = spawn(request).expect("spawn");
    std::env::remove_var("FLEET_WORKER_TEST_CHILD_EXE");
    let outcome = join(handle).expect("join");
    match outcome {
        LaneOutcome::Done { .. } => {}
        other => panic!("a worker that committed real work must be Done, got {other:?}"),
    }
    std::fs::remove_dir_all(&repo).ok();
}
