//! Shared real-git-repo scaffolding for fleet-merge's integration tests. Every repo lives under
//! a fresh `tempfile::TempDir`, never the checked-out repo tree.
#![allow(dead_code)] // not every test binary that includes this module uses every helper.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tempfile::TempDir;

/// Init a real git repo with one commit (`f.txt`) under a fresh tempdir. Returns the `TempDir`
/// guard (keep it alive) and the repo path.
pub fn init_repo() -> (TempDir, PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let repo = dir.path().to_path_buf();
    run(&repo, &["init", "-q"]);
    run(&repo, &["config", "user.email", "test@example.com"]);
    run(&repo, &["config", "user.name", "test"]);
    std::fs::write(repo.join("f.txt"), b"x").unwrap();
    run(&repo, &["add", "."]);
    run(&repo, &["commit", "-q", "-m", "init"]);
    (dir, repo)
}

pub fn run(repo: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("spawn git");
    assert!(status.success(), "git {args:?} failed in {repo:?}");
}

pub fn rev_parse_head(repo: &Path) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("spawn git");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}
