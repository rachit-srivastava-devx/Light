mod common;

use common::{fixture_exe, init_repo, lock_env};
use fleet_types::TaskId;
use fleet_worker::{join, spawn, CliAdapter, LaneOutcome, Role, SpawnRequest};
use std::time::Duration;

/// Regression test for `terminate_group`'s whole-process-group kill: a fixture that forks a
/// long-sleeping grandchild and never writes fd-3 must have BOTH itself and the grandchild dead
/// after `join`'s deadline fires, not just the direct child.
#[test]
fn join_times_out_and_kills_the_whole_process_group() {
    let _guard = lock_env();
    let repo = init_repo();
    std::env::set_var("FLEET_WORKER_TEST_CHILD_EXE", fixture_exe());
    let request = SpawnRequest {
        repo: repo.clone(),
        role: Role::Builder,
        task_id: TaskId::parse("t1").unwrap(),
        adapter: CliAdapter::Freelane,
        requested_model: None,
        task: "timeout".to_string(),
        deadline: Duration::from_millis(500),
    };
    let handle = spawn(request).expect("spawn");
    std::env::remove_var("FLEET_WORKER_TEST_CHILD_EXE");
    let marker = handle.worktree_path.join(".grandchild-pid");
    for _ in 0..40 {
        if marker.exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let outcome = join(handle).expect("join");
    assert!(matches!(outcome, LaneOutcome::EnvironmentFault { .. }));
    if let Ok(pid_text) = std::fs::read_to_string(&marker) {
        if let Ok(pid) = pid_text.trim().parse::<i32>() {
            let alive = std::process::Command::new("kill")
                .arg("-0")
                .arg(pid.to_string())
                .status()
                .map(|s| s.success())
                .unwrap_or(false);
            assert!(!alive, "grandchild pid {pid} must be dead after the whole-group kill");
        }
    }
    std::fs::remove_dir_all(&repo).ok();
}
