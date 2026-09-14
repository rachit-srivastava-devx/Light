//! `fleet gate` currently has the identical git-worktree wall `fleet run` had before node 2:
//! `gate()` (src/dispatch/verify/verify_cmd.rs) never touches git itself -- it only resolves the
//! gate table and runs it against `--repo` as cwd -- yet `ensure_repo` refuses a plain directory
//! unconditionally. Unlike `run`, there is no `Merge` stage here to skip: this is a strictly
//! smaller extension of node 2's `--no-git`, reusing `ensure_repo_mode` as-is.
//!
//! CONTRACT (pre-authored, RED until `--no-git` exists on `GateArgs`; do not edit to make it
//! pass, hard rule 1).

use std::path::Path;
use std::process::{Command, Output};

fn fleet() -> Command {
    Command::new(env!("CARGO_BIN_EXE_fleet"))
}

fn run_gate(repo: &Path, flags: &[&str]) -> Output {
    let state = tempfile::tempdir().expect("tempdir");
    fleet()
        .env("FLEET_STATE_DIR", state.path())
        .env("FLEET_LOAD_FACTOR", "10000")
        .args(["gate"])
        .args(flags)
        .arg("--repo")
        .arg(repo)
        .output()
        .expect("binary runs")
}

fn plain_dir_with_marker_gate() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(".fleet")).unwrap();
    std::fs::write(dir.path().join(".fleet/cwd-marker"), b"").unwrap();
    std::fs::write(
        dir.path().join(".fleet/gates.toml"),
        "[gates.\"unit tests\"]\ncommand = [\"bash\", \"-c\", \
         \"test -f .fleet/cwd-marker || exit 9; echo 'test result: ok. 1 passed; 0 failed;'\"]\n\
         probe = \"bash\"\n",
    )
    .unwrap();
    dir
}

/// Regression pin: without the flag, a plain directory still refuses exactly as before.
#[test]
fn plain_directory_without_the_flag_still_refuses() {
    let repo = plain_dir_with_marker_gate();
    let out = run_gate(repo.path(), &[]);
    assert_eq!(out.status.code(), Some(3));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("not a git repository"), "message unchanged: {stderr}");
}

/// The headline: `--no-git` lets `fleet gate` run against a plain directory, with the real gate
/// actually executing in that directory (marker-guarded, so a wrong cwd fails loudly instead of
/// quietly reporting a denominator it never earned).
#[test]
fn plain_directory_with_the_flag_runs_the_gate_for_real() {
    let repo = plain_dir_with_marker_gate();
    let out = run_gate(repo.path(), &["--no-git", "--id", "unit tests"]);
    assert!(
        out.status.success(),
        "exit={:?} stdout={} stderr={}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// `--no-git` on a real git worktree is still legal (gate never needed git to begin with, so
/// this is a true no-op there, not a special case).
#[test]
fn flag_on_a_real_git_worktree_is_still_legal() {
    let dir = tempfile::tempdir().expect("tempdir");
    let run = |args: &[&str]| {
        assert!(Command::new("git")
            .current_dir(dir.path())
            .args(args)
            .output()
            .expect("git runs")
            .status
            .success());
    };
    run(&["init", "-q"]);
    run(&["config", "user.email", "t@t.com"]);
    run(&["config", "user.name", "t"]);
    std::fs::create_dir_all(dir.path().join(".fleet")).unwrap();
    std::fs::write(dir.path().join(".fleet/cwd-marker"), b"").unwrap();
    std::fs::write(
        dir.path().join(".fleet/gates.toml"),
        "[gates.\"unit tests\"]\ncommand = [\"bash\", \"-c\", \
         \"test -f .fleet/cwd-marker || exit 9; echo 'test result: ok. 1 passed; 0 failed;'\"]\n\
         probe = \"bash\"\n",
    )
    .unwrap();
    let out = run_gate(dir.path(), &["--no-git", "--id", "unit tests"]);
    assert!(out.status.success(), "stderr={}", String::from_utf8_lossy(&out.stderr));
}
