//! Drives the real compiled `fleet` binary (BLUEPRINT §9) for the two "reports success while
//! failing" defects fixed alongside this file: an unknown `--id` must not be a silent no-op
//! `Ok`, and `rollback` on a worktree that never existed must not print `ok`. Both cases are
//! fast (no semgrep/trivy subprocess), unlike `fleet oracle`, so they run on every `cargo test`.

mod support;
use support::cmd;

#[test]
fn unknown_gate_id_exits_nonzero_and_names_the_id() {
    let out = cmd()
        .args(["gate", "--id", "definitely-not-a-real-gate"])
        .output()
        .expect("binary runs");
    assert!(!out.status.success(), "unknown gate id must not exit 0");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("definitely-not-a-real-gate"), "stderr should name the unknown id: {stderr}");
}

#[test]
fn rollback_of_a_nonexistent_worktree_exits_nonzero_and_does_not_claim_ok() {
    let path = std::env::temp_dir().join(format!("fleet-rollback-nope-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    let out = cmd()
        .args(["rollback", "--repo", ".", "--worktree", path.to_str().expect("utf8 path")])
        .output()
        .expect("binary runs");
    assert!(!out.status.success(), "rollback of nothing must not exit 0");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.contains("ok: worktree removed"), "must not claim success: {stdout}");
}
