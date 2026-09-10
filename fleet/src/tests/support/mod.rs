//! Shared helpers for the `__agent`/`__spawn_probe` integration tests, split out so each
//! `src/tests/*.rs` test file stays ≤80 lines. Not itself discovered as a Cargo test target --
//! `tests/support/` is a subdirectory, and Cargo only auto-discovers files directly in `tests/`.
#![allow(dead_code)]

pub mod bounded;
pub mod gates;
pub mod m4;

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

pub fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_fleet")
}

/// The ONE place that builds a `Command` for the real `fleet` binary in tests. Sets
/// `FLEET_LOAD_FACTOR` to a huge value so the machine-capacity preflight's LOAD component can
/// never refuse a test run because the CI/dev box happens to be busy -- a test suite whose result
/// depends on ambient load is non-deterministic. This configures the gate's threshold; it does not
/// disable the gate (see `capacity_refusal_real_binary.rs`, which drives the real refusal paths
/// with its own low, deliberate overrides). Every test that spawns the real binary should go
/// through this helper so a future test cannot forget the env var.
pub fn cmd() -> Command {
    let mut c = Command::new(bin());
    c.env("FLEET_LOAD_FACTOR", "10000");
    c
}

pub fn agent(args: &[&str]) -> Output {
    cmd().arg("__agent").args(args).output().expect("binary runs")
}

/// A throwaway repo with one committed file, so `fleet_merge::create` has something real to
/// branch a worktree from.
pub fn scratch_repo(dir: &Path) {
    let run = |args: &[&str]| {
        let out = Command::new("git").current_dir(dir).args(args).output().expect("git runs");
        assert!(out.status.success(), "git {args:?} failed: {}", String::from_utf8_lossy(&out.stderr));
    };
    run(&["init", "-q"]);
    run(&["config", "user.email", "test@example.com"]);
    run(&["config", "user.name", "test"]);
    fs::write(dir.join("f.txt"), "hello").unwrap();
    run(&["add", "f.txt"]);
    run(&["commit", "-q", "-m", "init"]);
}

/// Same as `scratch_repo`, but stops after `git add` -- `Merge`'s real `check_stage_nonempty`
/// sees a non-empty stage, so the pipeline's `Merge` stage (and every stage before it) succeeds.
pub fn scratch_repo_staged(dir: &Path) {
    let run = |args: &[&str]| {
        let out = Command::new("git").current_dir(dir).args(args).output().expect("git runs");
        assert!(out.status.success(), "git {args:?} failed: {}", String::from_utf8_lossy(&out.stderr));
    };
    run(&["init", "-q"]);
    run(&["config", "user.email", "test@example.com"]);
    run(&["config", "user.name", "test"]);
    fs::write(dir.join("f.txt"), "hello").unwrap();
    run(&["add", "f.txt"]);
}

/// Runs the hidden `__pipeline_probe` (real `Verify`, zero gates -- see `run_cmd.rs`'s own doc
/// comment on `NO_GATES`) against `state_dir`/`repo`, with `FLEET_STREAM_DIR` set to `stream_dir`
/// when given.
pub fn pipeline_probe(state_dir: &Path, repo: &Path, task_id: &str, stream_dir: Option<&Path>) -> Output {
    let mut c = cmd();
    c.env("FLEET_STATE_DIR", state_dir).args(["__pipeline_probe", "--task-id", task_id, "--repo"]).arg(repo);
    if let Some(dir) = stream_dir {
        c.env("FLEET_STREAM_DIR", dir);
    }
    c.output().expect("binary runs")
}
