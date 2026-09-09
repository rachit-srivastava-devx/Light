//! `$HOME` unset (and no `FLEET_STATE_DIR`, no `XDG_STATE_HOME`) must fail with a typed,
//! readable error and a non-zero exit -- never a panic and never a silent fallback into the CWD.
//! Drives the real binary so this proves the actual process behaviour, not just `ConfigError`'s
//! `Display` impl in isolation.

mod support;
use support::cmd;

fn git_status_porcelain(repo: &std::path::Path) -> String {
    let out = std::process::Command::new("git")
        .current_dir(repo)
        .args(["status", "--porcelain"])
        .output()
        .expect("git runs");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn missing_home_is_a_typed_error_not_a_panic() {
    let repo = tempfile::tempdir().unwrap();
    support::scratch_repo_staged(repo.path());
    let before = git_status_porcelain(repo.path());

    let out = cmd()
        .current_dir(repo.path())
        .env_remove("HOME")
        .env_remove("FLEET_STATE_DIR")
        .env_remove("XDG_STATE_HOME")
        .args(["__pipeline_probe", "--task-id", "state-dir-home-unset-task", "--repo"])
        .arg(repo.path())
        .output()
        .expect("binary runs");

    assert!(!out.status.success(), "must refuse, not silently pick a fallback dir");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("HOME is not set"), "expected the typed HOME-unset message, got: {stderr}");
    assert!(!stderr.contains("panicked at"), "must be a typed error, not a panic: {stderr}");

    // No `.fleet-state` (or any other new path) must appear in the repo despite the failure.
    let after = git_status_porcelain(repo.path());
    assert_eq!(before, after, "repo's git status must be unchanged even on refusal");
}
