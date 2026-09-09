mod common;

use common::{fixture_exe, init_repo, lock_env};
use fleet_types::TaskId;
use fleet_worker::{join, spawn, CliAdapter, LaneOutcome, MergePolicy, Role, SpawnRequest};
use std::time::Duration;

/// Direct regression test for the brief's "NEVER the user's `~/.claude`" requirement: set
/// `HOME`/`XDG_CONFIG_HOME` in THIS test process to sentinel values, spawn a fixture that dumps
/// its own environment, and assert the child saw neither the sentinel nor an unset value, but a
/// fresh per-lane tempdir this crate created.
#[test]
fn hermetic_spawn_never_exposes_real_home_or_xdg() {
    let _guard = lock_env();
    let repo = init_repo();
    let sentinel_home = "/tmp/SENTINEL-REAL-HOME-must-never-leak";
    let sentinel_xdg = "/tmp/SENTINEL-REAL-XDG-must-never-leak";
    let prior_home = std::env::var("HOME").ok();
    let prior_xdg = std::env::var("XDG_CONFIG_HOME").ok();
    std::env::set_var("HOME", sentinel_home);
    std::env::set_var("XDG_CONFIG_HOME", sentinel_xdg);
    // A non-allowlisted var: HOME/XDG alone can't prove `env_clear()` ran, since `.env("HOME",
    // ..)` overrides HOME regardless -- this sentinel proves the REST of the ambient
    // environment (credentials included) was actually wiped, not just HOME/XDG overridden.
    std::env::set_var("FLEET_WORKER_TEST_SENTINEL_LEAK", "must-never-reach-the-child");
    std::env::set_var("FLEET_WORKER_TEST_CHILD_EXE", fixture_exe());

    let request = SpawnRequest {
        repo: repo.clone(),
        role: Role::Builder,
        task_id: TaskId::parse("t1").unwrap(),
        adapter: CliAdapter::Freelane,
        requested_model: None,
        task: "envdump".to_string(),
        deadline: Duration::from_secs(10),
    };
    let handle = spawn(request).expect("spawn");
    std::env::remove_var("FLEET_WORKER_TEST_CHILD_EXE");
    std::env::remove_var("FLEET_WORKER_TEST_SENTINEL_LEAK");
    match prior_home {
        Some(v) => std::env::set_var("HOME", v),
        None => std::env::remove_var("HOME"),
    }
    match prior_xdg {
        Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
        None => std::env::remove_var("XDG_CONFIG_HOME"),
    }

    let (outcome, _merge) = join(handle, MergePolicy::Never).expect("join");
    let LaneOutcome::Done { body, .. } = outcome else {
        panic!("expected Done from envdump fixture, got {outcome:?}");
    };
    let seen_home = body["home"].as_str().unwrap_or_default();
    let seen_xdg = body["xdg_config_home"].as_str().unwrap_or_default();
    assert_ne!(seen_home, sentinel_home, "child must never see the real HOME");
    assert_ne!(seen_xdg, sentinel_xdg, "child must never see the real XDG_CONFIG_HOME");
    assert!(!seen_home.is_empty(), "child must see SOME HOME, not unset");
    assert!(seen_home.contains(".fleet-sandbox"), "HOME must be the per-lane tempdir");
    // The direct regression check for `env_clear()` itself: HOME/XDG override alone cannot
    // prove the REST of the ambient environment was wiped -- this sentinel can.
    assert_eq!(
        body["sentinel_leaked"], false,
        "a non-allowlisted var from the parent's real env reached the child -- env_clear() did not run"
    );
    let var_count = body["var_count"].as_u64().unwrap_or(u64::MAX);
    assert!(var_count <= 6, "hermetic child saw {var_count} env vars, expected at most the allowlist");

    std::fs::remove_dir_all(&repo).ok();
}
