//! Drives the REAL compiled `fleet` binary (never `FLEET_WORKER_TEST_CHILD_EXE`) to prove the
//! `__agent` bug is actually fixed: every spawned worker used to die instantly with
//! "unrecognized subcommand '__agent'" because `src/cli/root.rs` had no such variant, and all
//! 351 prior tests passed anyway because they substituted a fake binary for the child side,
//! never exercising this path (PRINCIPLES: a proxy is not the property). `env!("CARGO_BIN_EXE_
//! fleet")` -- verified against `src/Cargo.toml`'s `[[bin]] name = "fleet"` -- is Cargo's
//! compile-time path to the real product binary, not the test harness's own `current_exe()`.
//! The four typed-error-case tests live in `agent_child_error_cases.rs` (≤80-line split).

mod support;

use support::{agent, cmd, scratch_repo};

/// TASK 4(a): `FLEET_WORKER_TEST_CHILD_EXE` is a fault-injection-only seam (see
/// `crates/fleet-worker/src/spawn/util.rs`); this whole file exists to close the hole where the
/// suite went all-green using ONLY that fake binary and never exercised the real re-exec path.
#[test]
fn the_real_path_tests_do_not_run_under_the_fake_binary_seam() {
    assert!(
        std::env::var_os("FLEET_WORKER_TEST_CHILD_EXE").is_none(),
        "FLEET_WORKER_TEST_CHILD_EXE must not be set for this file's tests -- they exist \
         specifically to exercise the real `__agent` re-exec path, not the fixture one"
    );
}

/// clap's own usage-error exit code is 2; a real dispatch failure exits with one of
/// `fleet-types::ExitCode` (3/6/7/8/9), never 2, and never prints clap's usage message.
#[test]
fn agent_subcommand_is_recognized_by_the_real_binary() {
    let out = agent(&["freelane", "/tmp", "some task"]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!stderr.contains("unrecognized subcommand"), "stderr: {stderr}");
    assert_ne!(out.status.code(), Some(2), "must not fail as a clap usage error: {stderr}");
}

/// `__spawn_probe` drives `fleet_worker::spawn`/`join`'s real parent path (real worktree, real
/// re-exec of the real binary) end to end, WITHOUT `FLEET_WORKER_TEST_CHILD_EXE`. A `refused`
/// or `done` line proves a receipt genuinely came back over fd 3; the probe only reports "no
/// receipt" if `join` sees `EnvironmentFault`, and turns that into a nonzero exit.
#[test]
fn a_real_spawn_gets_a_real_fd3_receipt_back() {
    let repo = tempfile::tempdir().unwrap();
    scratch_repo(repo.path());
    let out = cmd()
        .args(["__spawn_probe", "--repo"])
        .arg(repo.path())
        .output()
        .expect("binary runs");
    assert!(out.status.success(), "spawn probe failed: {}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("__spawn_probe: done") || stdout.contains("__spawn_probe: refused:"),
        "no receipt came back over fd 3: stdout={stdout} stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
}
