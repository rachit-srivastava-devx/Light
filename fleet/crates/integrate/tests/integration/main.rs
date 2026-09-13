use integrate::{integrate, Grant, IntegrateError, MergeRequest, RealGit};
use std::path::Path;
use tempfile::TempDir;

fn git_in(dir: &Path, args: &[&str]) {
    std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t.com")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t.com")
        .output()
        .expect("git command failed");
}

fn head_of(dir: &Path) -> String {
    let o = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(dir)
        .output()
        .expect("rev-parse HEAD");
    String::from_utf8(o.stdout).unwrap().trim().to_string()
}

fn grant(t: &str, d: &str) -> Grant {
    Grant {
        target_ref: t.into(),
        candidate_digest: d.into(),
        expires_at: 9_999_999_999,
    }
}

#[test]
fn moved_head_refuses() {
    let d = TempDir::new().unwrap();
    let p = d.path();
    git_in(p, &["init", "-b", "main"]);
    std::fs::write(p.join("a.txt"), "init\n").unwrap();
    git_in(p, &["add", "."]);
    git_in(p, &["commit", "-m", "init"]);
    let stale = head_of(p);
    std::fs::write(p.join("b.txt"), "next\n").unwrap();
    git_in(p, &["add", "."]);
    git_in(p, &["commit", "-m", "next"]);
    let req = MergeRequest {
        repo: p.to_path_buf(),
        expected_head: stale,
        lane_head: "main".into(),
        candidate_digest: "d1".into(),
        grant: grant("main", "d1"),
        checked: 1,
        total: 1,
    };
    assert!(matches!(
        integrate(&RealGit, req),
        Err(IntegrateError::CasMismatch { .. })
    ));
}

#[test]
fn conflict_produces_refusal() {
    let d = TempDir::new().unwrap();
    let p = d.path();
    git_in(p, &["init", "-b", "main"]);
    std::fs::write(p.join("f.txt"), "base\n").unwrap();
    git_in(p, &["add", "."]);
    git_in(p, &["commit", "-m", "base"]);
    git_in(p, &["checkout", "-b", "feat"]);
    std::fs::write(p.join("f.txt"), "feature\n").unwrap();
    git_in(p, &["add", "."]);
    git_in(p, &["commit", "-m", "feat"]);
    git_in(p, &["checkout", "main"]);
    std::fs::write(p.join("f.txt"), "main-change\n").unwrap();
    git_in(p, &["add", "."]);
    git_in(p, &["commit", "-m", "main-change"]);
    let req = MergeRequest {
        repo: p.to_path_buf(),
        expected_head: head_of(p),
        lane_head: "feat".into(),
        candidate_digest: "d2".into(),
        grant: grant("feat", "d2"),
        checked: 1,
        total: 1,
    };
    assert!(matches!(
        integrate(&RealGit, req),
        Err(IntegrateError::Conflict { .. })
    ));
}
