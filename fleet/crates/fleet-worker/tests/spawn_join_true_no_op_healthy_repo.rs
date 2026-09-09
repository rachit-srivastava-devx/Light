//! The gap `docs/VERIFY-APPLY-HONESTY.md` identifies: no existing test runs a true no-op `Done`
//! through the REAL `spawn()` -> `join()` pipeline against a HEALTHY git repo. The unit test
//! `change_detect::tests::a_true_no_op_is_still_refused` calls `enforce_change_honesty` directly
//! against a bare `tempdir()` that never saw `record_worker_pid` write `.fleet-lane.pid` into it.
//! `spawn_join_git_env_fault.rs` does go through the real pipeline with the "done_no_change"
//! fixture, but deliberately breaks `git` first, so it never reaches the healthy-repo
//! `dirty_file_count` call. Neither one can observe `.fleet-lane.pid` fooling `git status` into
//! reporting a dirty tree for a lane that did nothing.
//!
//! Before the fix (`reap::clear_worker_pid` called from `join_impl` right before the honesty
//! check), this test fails: `.fleet-lane.pid` alone makes `dirty_file_count() > 0`, so a true
//! no-op reads as `Done` instead of `Refused`. After the fix, it passes.
mod common;

use common::{fixture_exe, init_repo, lock_env};
use fleet_types::TaskId;
use fleet_worker::{join, spawn, CliAdapter, LaneOutcome, Role, SpawnRequest};
use std::time::Duration;

#[test]
fn a_true_no_op_through_the_real_lifecycle_against_a_healthy_repo_is_refused() {
    let _guard = lock_env();
    let repo = init_repo();
    std::env::set_var("FLEET_WORKER_TEST_CHILD_EXE", fixture_exe());
    let request = SpawnRequest {
        repo: repo.clone(),
        role: Role::Builder,
        task_id: TaskId::parse("t1").unwrap(),
        adapter: CliAdapter::Freelane,
        requested_model: None,
        task: "done_no_change".to_string(),
        deadline: Duration::from_secs(10),
    };
    let handle = spawn(request).expect("spawn");
    std::env::remove_var("FLEET_WORKER_TEST_CHILD_EXE");

    // `git` is untouched here (unlike spawn_join_git_env_fault.rs) -- this is the healthy path.
    let outcome = join(handle).expect("join");

    match outcome {
        LaneOutcome::Refused { reason } => {
            assert!(reason.contains("unchanged"), "unexpected refusal reason: {reason}");
        }
        other => panic!(
            "a worker that touched nothing must be Refused, not {other:?} -- fleet's own \
             `.fleet-lane.pid` bookkeeping must never count as the worker's change"
        ),
    }
    std::fs::remove_dir_all(&repo).ok();
}
