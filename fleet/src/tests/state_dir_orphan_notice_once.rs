//! When a CWD-relative `.fleet-state` pre-exists (left over from before Gap 1's fix, or from an
//! explicit override that used to point there) and the relocated default is used instead, fleet
//! must say so exactly ONCE on stderr, naming both paths -- not silently, and not auto-migrating
//! the user's ledger. "Once" here means once per process invocation, which is what this test
//! pins: a single command run must print the notice exactly one time, never duplicated per line
//! of its own output.

mod support;
use support::cmd;

#[test]
fn stale_cwd_state_prints_the_relocation_notice_exactly_once() {
    let repo = tempfile::tempdir().unwrap();
    support::scratch_repo_staged(repo.path());
    std::fs::create_dir_all(repo.path().join(".fleet-state")).unwrap();
    let fake_home = tempfile::tempdir().unwrap();

    let out = cmd()
        .current_dir(repo.path())
        .env("HOME", fake_home.path())
        .env_remove("FLEET_STATE_DIR")
        .env_remove("XDG_STATE_HOME")
        .args(["__pipeline_probe", "--task-id", "state-dir-notice-task", "--repo"])
        .arg(repo.path())
        .output()
        .expect("binary runs");
    assert!(out.status.success(), "probe failed: {}", String::from_utf8_lossy(&out.stderr));

    let stderr = String::from_utf8_lossy(&out.stderr);
    let occurrences = stderr.matches("found existing state at").count();
    assert_eq!(occurrences, 1, "notice must print exactly once, got {occurrences} in: {stderr}");
    assert!(stderr.contains(".fleet-state"), "notice must name the old CWD-relative path: {stderr}");
    assert!(stderr.contains("fleet"), "notice must name the new default path: {stderr}");
}
