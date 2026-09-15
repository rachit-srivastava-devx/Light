//! Verify-node E2E matrix. Every case invokes the compiled fleet binary and uses a real repo.
#[path = "../support/mod.rs"]
mod support;

use std::fs;
use std::process::Output;
use support::cmd;

fn repo_with_test(body: &str) -> tempfile::TempDir {
    let repo = tempfile::tempdir().unwrap();
    support::scratch_repo(repo.path());
    fs::write(
        repo.path().join("Cargo.toml"),
        "[package]\nname = \"golden-verify-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    fs::create_dir_all(repo.path().join("src")).unwrap();
    fs::write(
        repo.path().join("src/lib.rs"),
        format!("#[test]\nfn t() {{ {body} }}\n"),
    )
    .unwrap();
    repo
}

fn gate(repo: &std::path::Path) -> Output {
    // The real unit-tests gate builds the workspace on a cold target. Keep this
    // bounded, but leave enough budget for a genuine end-to-end invocation.
    cmd()
        .env("FLEET_VERIFY_BUDGET_SECS", "180")
        .args(["gate", "--id", "unit tests", "--repo"])
        .arg(repo)
        .output()
        .unwrap()
}

#[test]
fn real_binary_reports_pass_with_measured_denominator() {
    let repo = repo_with_test("assert_eq!(2 + 2, 4);");
    let out = gate(repo.path());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{err}");
    assert!(err.contains("1/1"), "missing measured denominator: {err}");
}

#[test]
fn real_binary_reports_failure_for_real_repo_test() {
    let repo = repo_with_test("assert!(false, \"matrix failure\");");
    let out = gate(repo.path());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "a failing real test must fail: {err}"
    );
    assert!(err.contains("0/1"), "missing failure denominator: {err}");
}

#[test]
fn real_binary_redacts_a_planted_secret() {
    let repo = repo_with_test("assert!(true);");
    fs::write(
        repo.path().join("secret.txt"),
        "api_key = \"fleet-test-secret-1234567890\"\n",
    )
    .unwrap();
    let committed = std::process::Command::new("git")
        .current_dir(repo.path())
        .args(["add", "secret.txt"])
        .status()
        .unwrap();
    assert!(committed.success());
    let committed = std::process::Command::new("git")
        .current_dir(repo.path())
        .args(["commit", "-qm", "fixture secret"])
        .status()
        .unwrap();
    assert!(committed.success());
    let out = gate(repo.path());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "planted secret must block: {err}");
    assert!(
        err.contains("redacted=true"),
        "redaction evidence missing: {err}"
    );
    assert!(
        !err.contains("fleet-test-secret-1234567890"),
        "secret leaked: {err}"
    );
}
