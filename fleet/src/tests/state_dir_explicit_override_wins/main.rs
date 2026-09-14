//! `FLEET_STATE_DIR` must keep working exactly as it did before Gap 1's fix: an explicit
//! override always wins over the (now relocated) default. Pins that the override still lands
//! state exactly where asked, and specifically NOT under the per-user default, so a future
//! change to the default can never silently swallow the override.

use std::path::Path;

#[path = "../support/mod.rs"]
mod support;
use support::cmd;

#[test]
fn flee_state_dir_env_var_overrides_the_default_and_state_lands_there() {
    let repo = tempfile::tempdir().unwrap();
    support::scratch_repo_staged(repo.path());
    let fake_home = tempfile::tempdir().unwrap();
    let explicit_state = tempfile::tempdir().unwrap();
    let task_id = "state-dir-override-task";

    let out = cmd()
        .current_dir(repo.path())
        .env("HOME", fake_home.path())
        .env("FLEET_STATE_DIR", explicit_state.path())
        .env_remove("XDG_STATE_HOME")
        .args(["__pipeline_probe", "--task-id", task_id, "--repo"])
        .arg(repo.path())
        .output()
        .expect("binary runs");
    assert!(
        out.status.success(),
        "probe failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    assert!(
        support::step_log_path(explicit_state.path(), task_id).is_some(),
        "explicit FLEET_STATE_DIR must be honored: no step log for {task_id:?} under {:?}",
        explicit_state.path()
    );

    let default_dir = fake_home.path().join(".local").join("state").join("fleet");
    assert!(
        !Path::new(&default_dir).exists(),
        "override must win; default dir must stay untouched"
    );
}
