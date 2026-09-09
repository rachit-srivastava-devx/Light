//! Gap 1 regression: `Config::state_dir`'s default used to be the CWD-relative `.fleet-state`,
//! so a state-writing command run with the CWD inside a repo (the normal case) left an untracked
//! `.fleet-state/` in `git status`. The default must now be a per-user location, never under the
//! CWD, matching `fleet-worker::spawn::worker_state_dir`'s fallback shape.
//!
//! Drives the REAL binary (`support::cmd`), never a substituted fake. `HOME` is pinned to a
//! throwaway tempdir (and `XDG_STATE_HOME` cleared) so the default resolves somewhere this test
//! controls and can assert on, regardless of the ambient environment running the suite.

use std::fs;
use std::path::Path;

mod support;
use support::cmd;

fn run_probe(repo: &Path, fake_home: &Path, task_id: &str) -> std::process::Output {
    cmd()
        .current_dir(repo)
        .env("HOME", fake_home)
        .env_remove("FLEET_STATE_DIR")
        .env_remove("XDG_STATE_HOME")
        .args(["__pipeline_probe", "--task-id", task_id, "--repo"])
        .arg(repo)
        .output()
        .expect("binary runs")
}

fn git_status_porcelain(repo: &Path) -> String {
    let out = std::process::Command::new("git")
        .current_dir(repo)
        .args(["status", "--porcelain"])
        .output()
        .expect("git runs");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn default_state_dir_never_lands_in_the_repo_and_is_the_per_user_default() {
    let repo = tempfile::tempdir().unwrap();
    support::scratch_repo_staged(repo.path());
    let fake_home = tempfile::tempdir().unwrap();
    let task_id = "state-dir-default-task";

    // `scratch_repo_staged` intentionally leaves one file staged-but-uncommitted (`Merge`'s
    // `check_stage_nonempty` needs that) -- so "clean" here means "unchanged by this run", not
    // "empty", or this test would conflate that pre-existing staged file with the defect.
    let before = git_status_porcelain(repo.path());

    let out = run_probe(repo.path(), fake_home.path(), task_id);
    assert!(out.status.success(), "probe failed: {}", String::from_utf8_lossy(&out.stderr));

    // The defect: an untracked `.fleet-state/` (or any other new path) appearing in the repo.
    let after = git_status_porcelain(repo.path());
    assert_eq!(before, after, "repo's git status must be unchanged by the run");
    assert!(!repo.path().join(".fleet-state").exists(), "must not create .fleet-state in the repo");

    // The fix: state actually lands under the per-user default, `$HOME/.local/state/fleet`.
    let expected_dir = fake_home.path().join(".local").join("state").join("fleet");
    let log_path = expected_dir.join(format!("{task_id}.steps.json"));
    assert!(log_path.exists(), "expected step log at {log_path:?}, dir contents: {:?}", fs::read_dir(&expected_dir).ok().map(|d| d.filter_map(|e| e.ok().map(|e| e.file_name())).collect::<Vec<_>>()));
}
