use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;
use verify::{FindingsProvider, GitleaksFindingsProvider};

fn git(repo: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .expect("git exists");
    assert!(output.status.success(), "git failed: {:?}", args);
}

#[test]
fn real_gitleaks_output_is_parsed_and_redacted() {
    let repo = tempdir().expect("temp repo");
    fs::write(
        repo.path().join("fixture.txt"),
        "api_key = \"fleet-test-secret-1234567890\"\n",
    )
    .expect("fixture writes");
    git(repo.path(), &["init", "-q"]);
    git(repo.path(), &["config", "user.email", "fleet@test.invalid"]);
    git(repo.path(), &["config", "user.name", "Fleet Test"]);
    git(repo.path(), &["add", "fixture.txt"]);
    git(repo.path(), &["commit", "-qm", "fixture"]);

    let findings = GitleaksFindingsProvider::new(repo.path())
        .scan("tree-under-test")
        .expect("installed gitleaks must produce JSON");
    assert!(!findings.is_empty(), "fixture must exercise the scanner");
    assert!(findings.iter().all(|finding| finding.redacted));
    assert!(findings.iter().all(|finding| {
        finding.file.contains("fixture.txt") && !format!("{finding:?}").contains("fleet-test")
    }));
}
