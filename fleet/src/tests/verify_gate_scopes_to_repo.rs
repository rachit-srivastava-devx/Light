//! S1 proof: `fleet gate`/`fleet oracle` used to spawn every gate with no `.current_dir()`, so
//! `cargo test --workspace` ran against the process's own cwd, not the `--repo` named. Same cwd
//! for every case below, two `--repo` targets, two correct verdicts: a scratch crate with one
//! failing test FAILs (never compiling fleet's own tree), the same crate fixed PASSes, and a
//! nonexistent `--repo` is refused up front with a typed error and no verdict at all.

use std::fs;
use std::path::Path;
use std::process::Output;
use std::time::Instant;

mod support;
use support::cmd;

fn write_scratch_crate(dir: &Path, test_body: &str) {
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(dir.join("Cargo.toml"), "[package]\nname=\"scratch\"\nversion=\"0.1.0\"\nedition=\"2021\"\n").unwrap();
    fs::write(dir.join("src/lib.rs"), format!("#[test]\nfn scratch_test() {{\n{test_body}\n}}\n")).unwrap();
}

fn run_gate(repo: &Path) -> (Output, std::time::Duration) {
    let started = Instant::now();
    let out = cmd()
        .env("FLEET_VERIFY_BUDGET_SECS", "60")
        .args(["gate", "--id", "unit tests", "--repo"])
        .arg(repo)
        .output()
        .expect("binary runs");
    (out, started.elapsed())
}

#[test]
fn a_scratch_repos_failing_test_is_reported_without_compiling_fleets_own_tree() {
    let repo = tempfile::tempdir().unwrap();
    write_scratch_crate(repo.path(), "assert!(false, \"deliberately failing\");");
    let (out, elapsed) = run_gate(repo.path());
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(!out.status.success(), "a real failing test in --repo must fail the gate: {stderr}");
    // The SCRATCH crate has exactly ONE test, so a `0/1` denominator (not fleet's own ~463-test
    // count) is the proof this ran against `--repo`, not the fleet workspace this binary lives in.
    assert!(stderr.contains("0/1"), "must report the SCRATCH crate's own 1-test denominator: {stderr}");
    assert!(!stderr.contains("fleet-types"), "must not compile fleet's own workspace: {stderr}");
    assert!(!stderr.contains("fleet-cli"), "must not compile fleet's own workspace: {stderr}");
    eprintln!("elapsed scanning the failing scratch repo: {elapsed:?}");
}

#[test]
fn the_same_scratch_repo_with_a_passing_test_reports_pass() {
    let repo = tempfile::tempdir().unwrap();
    write_scratch_crate(repo.path(), "assert_eq!(2 + 2, 4);");
    let (out, elapsed) = run_gate(repo.path());
    let stderr = String::from_utf8_lossy(&out.stderr);

    assert!(out.status.success(), "a real passing test in --repo must pass the gate: {stderr}");
    assert!(stderr.contains("1/1"), "must report the SCRATCH crate's own 1-test denominator: {stderr}");
    assert!(!stderr.contains("fleet-types"), "must not compile fleet's own workspace: {stderr}");
    eprintln!("elapsed scanning the passing scratch repo: {elapsed:?}");
}

#[test]
fn an_unusable_repo_is_a_typed_refusal_never_a_verdict_about_a_different_directory() {
    let bogus = "/definitely/not/here/fleet-s1-proof";
    let out = cmd().args(["gate", "--repo", bogus]).output().expect("binary runs");
    assert!(!out.status.success(), "a nonexistent --repo must never exit 0");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains(bogus), "the typed error must name the bad repo: {stderr}");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.is_empty(), "no verdict should print for a refused target: {stdout}");
}
