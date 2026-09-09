mod common;

use common::{fixture_exe, init_repo, lock_env};
use fleet_types::TaskId;
use fleet_worker::{join, spawn, CliAdapter, LaneOutcome, MergePolicy, Role, SpawnError, SpawnRequest};
use std::time::Duration;

fn base_request(repo: &std::path::Path, task: &str) -> SpawnRequest {
    SpawnRequest {
        repo: repo.to_path_buf(),
        role: Role::Builder,
        task_id: TaskId::parse("t1").unwrap(),
        adapter: CliAdapter::Freelane,
        requested_model: None,
        task: task.to_string(),
        deadline: Duration::from_secs(3),
    }
}

#[test]
fn spawn_join_empty_task_refuses_before_any_io() {
    let _guard = lock_env();
    let repo = init_repo();
    let worktrees_before = std::fs::read_dir(repo.join(".worktrees")).ok();
    let err = spawn(base_request(&repo, "   ")).unwrap_err();
    assert_eq!(err, SpawnError::EmptyTask);
    let after_count = std::fs::read_dir(repo.join(".worktrees")).map(|d| d.count()).unwrap_or(0);
    let before_count = worktrees_before.map(|d| d.count()).unwrap_or(0);
    assert_eq!(after_count, before_count, "no worktree may be created for a refused spawn");
    std::fs::remove_dir_all(&repo).ok();
}

#[test]
fn spawn_join_missing_cli_refuses_before_any_io() {
    let _guard = lock_env();
    let repo = init_repo();
    let mut request = base_request(&repo, "done");
    request.adapter = CliAdapter::Claude;
    // This dev/CI host may genuinely have a real `claude` on PATH -- clear PATH for the
    // duration of this one call so the presence check has something to actually refuse. The
    // CLI-presence check runs before any worktree IO, so `git` never needs to be resolved here.
    let prior_path = std::env::var_os("PATH");
    std::env::remove_var("PATH");
    let err = spawn(request).unwrap_err();
    if let Some(path) = prior_path {
        std::env::set_var("PATH", path);
    }
    assert_eq!(err, SpawnError::CliNotOnPath(CliAdapter::Claude, "claude"));
    let count = std::fs::read_dir(repo.join(".worktrees")).map(|d| d.count()).unwrap_or(0);
    assert_eq!(count, 0, "no worktree may be created before the CLI-presence check runs");
    std::fs::remove_dir_all(&repo).ok();
}

#[test]
fn join_rejects_malformed_fd3_packet_as_environment_fault_not_refusal() {
    let _guard = lock_env();
    let repo = init_repo();
    std::env::set_var("FLEET_WORKER_TEST_CHILD_EXE", fixture_exe());
    let handle = spawn(base_request(&repo, "malformed")).expect("spawn");
    std::env::remove_var("FLEET_WORKER_TEST_CHILD_EXE");
    let (outcome, _merge) = join(handle, MergePolicy::Never).expect("join");
    match outcome {
        LaneOutcome::EnvironmentFault { .. } => {}
        LaneOutcome::Refused { .. } => panic!("malformed packet must never be classified Refused"),
        other => panic!("expected EnvironmentFault, got {other:?}"),
    }
    std::fs::remove_dir_all(&repo).ok();
}
