//! S2 end-to-end: if `git` itself is unavailable when `join`'s change-honesty check runs, that
//! must surface as `EnvironmentFault`, never a false `Refused` (AGENTS.md rule 7 -- an
//! environment fault must never be reported as an agent failure).
//!
//! `join()` also calls `git` again afterwards, unrelated to the honesty check, to tear the
//! worktree down (`fleet_merge::remove`) -- breaking `PATH` for the whole call would fail that
//! step too and mask the outcome under test behind a teardown error instead. So this uses the
//! test-only seam `change_detect::detect::GIT_OVERRIDE_ENV` (a bogus program name that only the
//! honesty check's own git invocations consult) rather than touching `PATH` at all.
//!
//! NOTE: because `git` is broken here on purpose, this test never reaches the real, healthy-repo
//! `dirty_file_count` call -- it does NOT cover whether a true no-op is refused on a healthy
//! repo. See `spawn_join_true_no_op_healthy_repo.rs` for that.
mod common;

use common::{fixture_exe, init_repo, lock_env};
use fleet_types::TaskId;
use fleet_worker::{join, spawn, CliAdapter, LaneOutcome, Role, SpawnRequest};
use std::time::Duration;

const GIT_OVERRIDE_ENV: &str = "FLEET_WORKER_TEST_CHANGE_DETECT_GIT";

#[test]
fn git_unavailable_during_the_honesty_check_is_an_environment_fault_not_a_refusal() {
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

    std::env::set_var(GIT_OVERRIDE_ENV, "definitely-not-a-real-git-binary-xyz");
    let outcome = join(handle).expect("join");
    std::env::remove_var(GIT_OVERRIDE_ENV);

    match outcome {
        LaneOutcome::EnvironmentFault { .. } => {}
        LaneOutcome::Refused { .. } => {
            panic!("git being unavailable must never be reported as the worker's refusal")
        }
        other => panic!("expected EnvironmentFault, got {other:?}"),
    }
    std::fs::remove_dir_all(&repo).ok();
}
