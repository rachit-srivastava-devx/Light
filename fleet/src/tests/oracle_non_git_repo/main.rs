//! `fleet oracle` has the identical shape to `fleet gate` (src/dispatch/verify/verify_cmd.rs):
//! it never touches git itself, only resolves the gate table and runs it against `--repo` as
//! cwd. Same extension as `gate_non_git_repo`, reusing `ensure_repo_mode` as-is.
//!
//! CONTRACT (pre-authored, RED until `--no-git` exists on `OracleArgs`; do not edit to make it
//! pass, hard rule 1).

use std::path::Path;
use std::process::{Command, Output};

fn fleet() -> Command {
    Command::new(env!("CARGO_BIN_EXE_fleet"))
}

fn run_oracle(repo: &Path, flags: &[&str]) -> Output {
    let state = tempfile::tempdir().expect("tempdir");
    fleet()
        .env("FLEET_STATE_DIR", state.path())
        .env("FLEET_LOAD_FACTOR", "10000")
        .args(["oracle"])
        .args(flags)
        .arg("--repo")
        .arg(repo)
        .output()
        .expect("binary runs")
}

fn plain_dir_all_gates_pass() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(".fleet")).unwrap();
    std::fs::write(dir.path().join(".fleet/cwd-marker"), b"").unwrap();
    let echo = |id: &str, line: &str| {
        format!(
            "[gates.\"{id}\"]\ncommand = [\"bash\", \"-c\", \"test -f .fleet/cwd-marker || \
             exit 9; echo '{line}'\"]\nprobe = \"bash\"\n\n"
        )
    };
    // All 8 registry gates overridden, same payloads `run_non_git_repo`'s fixture already
    // proved parse correctly -- `oracle` runs the FULL table (no `--id` filter), and this repo's
    // own `Requirement::Required` design means a skipped-as-not-applicable gate (detectors,
    // corpus) still fails the aggregate, same as a git repo with no matching input would. This
    // is not a --no-git-specific limitation, so the fixture must give every Required gate
    // something real to pass, not rely on self-skip. recur-gate.sh in particular *needs*
    // overriding here: without git it exits 3 (environment fault), which the aggregate counts
    // as a plain FAIL rather than a skip -- a real gap, flagged separately (task_25f563d4).
    let mut toml = String::new();
    toml.push_str(&echo("unit tests", "test result: ok. 1 passed; 0 failed;"));
    toml.push_str(&echo("mutants", "mutants: caught=2 total=2"));
    toml.push_str(&echo("semgrep", "10 files scanned, 0 findings"));
    toml.push_str(&echo("trivy", "0 secret findings across 4 reported targets"));
    toml.push_str(&echo("recur", "recur-gate: checked=1 flagged=0"));
    toml.push_str(&echo("detectors", "7 detectors match the manifest (denominator: 7)"));
    toml.push_str(&echo("policy", "-- 3 passed, 0 failed (denominator: 3 policies) --"));
    toml.push_str(&echo("corpus", "DENOMINATOR checked=9 total=9 caught=0"));
    std::fs::write(dir.path().join(".fleet/gates.toml"), toml).unwrap();
    dir
}

/// Regression pin: without the flag, a plain directory still refuses exactly as before.
#[test]
fn plain_directory_without_the_flag_still_refuses() {
    let repo = plain_dir_all_gates_pass();
    let out = run_oracle(repo.path(), &[]);
    assert_eq!(out.status.code(), Some(3));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("not a git repository"), "message unchanged: {stderr}");
}

/// The headline: `--no-git` lets `fleet oracle` run every real (non-skipped) gate against a
/// plain directory, with the marker-guarded commands proving it ran in the right cwd.
#[test]
fn plain_directory_with_the_flag_runs_every_configured_gate() {
    let repo = plain_dir_all_gates_pass();
    let out = run_oracle(repo.path(), &["--no-git"]);
    assert!(
        out.status.success(),
        "exit={:?} stdout={} stderr={}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}
