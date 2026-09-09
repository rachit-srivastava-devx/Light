//! `MergePolicy::Never` regression pin (real temp git repo, this crate's convention): a `Done`
//! lane joined with `Never` must never touch the target repo's `HEAD` -- the guarantee that
//! wiring merge-back in did not turn it on by accident.
mod common;

use common::{fixture_exe, head, init_repo, lock_env};
use fleet_types::TaskId;
use fleet_worker::{join, spawn, CliAdapter, LaneOutcome, MergePolicy, Role, SpawnRequest};
use std::time::Duration;

#[test]
fn merge_policy_never_leaves_target_head_unchanged() {
    let _guard = lock_env();
    let repo = init_repo();
    let before = head(&repo);
    std::env::set_var("FLEET_WORKER_TEST_CHILD_EXE", fixture_exe());
    let request = SpawnRequest {
        repo: repo.clone(),
        role: Role::Builder,
        task_id: TaskId::parse("t1").unwrap(),
        adapter: CliAdapter::Freelane,
        requested_model: None,
        task: "done".to_string(),
        deadline: Duration::from_secs(10),
    };
    let handle = spawn(request).expect("spawn");
    std::env::remove_var("FLEET_WORKER_TEST_CHILD_EXE");

    let (outcome, merge) = join(handle, MergePolicy::Never).expect("join");
    assert!(matches!(outcome, LaneOutcome::Done { .. }), "{outcome:?}");
    assert!(merge.is_none(), "Never must never produce a MergeOutcome");
    assert_eq!(head(&repo), before, "MergePolicy::Never must never move the target repo's HEAD");
    std::fs::remove_dir_all(&repo).ok();
}
