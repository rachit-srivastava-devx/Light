//! Task 1 regression: a no-op lane through the REAL `spawn()` -> `join()` pipeline must leave the
//! repo's `git status --porcelain --untracked-files=all` byte-identical to before the run --
//! fleet's own bookkeeping (scorecard state, `.fleet-sandbox/`, `.fleet-lane.pid`) must never be
//! among the counted artifacts. Before the fix, `join_impl::join` wrote the scorecard to
//! `<repo>/.fleet/state/scorecards/<agent>.json`, which showed up as `?? .fleet/` here.
mod common;

use common::{fixture_exe, init_repo, lock_env};
use fleet_types::TaskId;
use fleet_worker::{join, spawn, CliAdapter, LaneOutcome, Role, SpawnRequest};
use std::process::Command;
use std::time::Duration;

fn porcelain(repo: &std::path::Path) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["status", "--porcelain", "--untracked-files=all"])
        .output()
        .expect("git status");
    String::from_utf8_lossy(&out.stdout).to_string()
}

#[test]
fn a_no_op_run_leaves_zero_fleet_owned_artifacts_in_the_repo() {
    let _guard = lock_env();
    let repo = init_repo();
    let state_dir = tempfile::tempdir().unwrap();
    std::env::set_var("FLEET_WORKER_TEST_CHILD_EXE", fixture_exe());
    std::env::set_var("FLEET_STATE_DIR", state_dir.path());

    let before = porcelain(&repo);
    assert_eq!(before, "", "repo must start clean: {before}");

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
    let outcome = join(handle).expect("join");
    std::env::remove_var("FLEET_STATE_DIR");
    assert!(matches!(outcome, LaneOutcome::Refused { .. }), "{outcome:?}");

    let after = porcelain(&repo);
    assert_eq!(after, "", "a no-op lane must leave the repo exactly as clean as before: {after}");

    // And the scorecard was genuinely written -- just not inside the repo.
    let scorecard = state_dir.path().join("scorecards").join("builder.json");
    assert!(scorecard.exists(), "scorecard should still be recorded, just outside the repo");

    std::fs::remove_dir_all(&repo).ok();
}
