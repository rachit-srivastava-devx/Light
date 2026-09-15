use super::super::GitleaksFindingsProvider;
use crate::FindingsProvider;
use std::path::Path;
use std::time::Duration;

pub(super) fn assert_fallback(binary: &Path) {
    let repo = tempfile::tempdir().expect("temporary repository");
    std::process::Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(repo.path())
        .status()
        .expect("git init");
    std::fs::write(repo.path().join("fixture.txt"), "fixture").expect("fixture writes");
    let provider = GitleaksFindingsProvider::with_binary(repo.path(), binary)
        .with_budget(Duration::from_secs(30))
        .with_git_required(true);
    provider
        .scan("tree")
        .expect("unborn repo scans as directory");
    let scope = provider.last_scope().expect("scope published");
    assert_eq!(scope.total(), 1);
    assert!(!scope.git_backed());
    assert!(scope.requested_git());
    assert!(scope.no_head_fallback());
}
