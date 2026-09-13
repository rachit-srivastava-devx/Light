use integrate::{integrate, Grant, MergeRequest, RealGit};
use std::path::Path;

fn git_in(dir: &Path, args: &[&str]) {
    std::process::Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t.com")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t.com")
        .output()
        .expect("git");
}

fn head_of(dir: &Path) -> String {
    let o = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(dir)
        .output()
        .expect("rev-parse HEAD");
    String::from_utf8(o.stdout).unwrap().trim().to_string()
}

/// After a successful merge, `r.after` must equal the real post-merge HEAD.
/// Kills rev_parse_head → Ok("") / Ok("xyzzy") and merge → Ok("") / Ok("xyzzy").
#[test]
fn after_sha_matches_actual_head_post_merge() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    git_in(p, &["init", "-b", "main"]);
    std::fs::write(p.join("x.txt"), "base\n").unwrap();
    git_in(p, &["add", "."]);
    git_in(p, &["commit", "-m", "base"]);
    git_in(p, &["checkout", "-b", "lane"]);
    std::fs::write(p.join("y.txt"), "new\n").unwrap();
    git_in(p, &["add", "."]);
    git_in(p, &["commit", "-m", "lane"]);
    git_in(p, &["checkout", "main"]);
    let before = head_of(p);
    let req = MergeRequest {
        repo: p.to_path_buf(),
        expected_head: before.clone(),
        lane_head: "lane".into(),
        candidate_digest: "d3".into(),
        grant: Grant {
            target_ref: "lane".into(),
            candidate_digest: "d3".into(),
            expires_at: 9_999_999_999,
        },
        checked: 1,
        total: 1,
    };
    let r = integrate(&RealGit, req).expect("clean merge must succeed");
    let actual_head = head_of(p);
    assert_eq!(
        r.after, actual_head,
        "after must equal the real post-merge HEAD SHA"
    );
    assert_ne!(r.after, before, "after must differ from pre-merge HEAD");
    assert_ne!(r.after, "", "after must not be empty");
    assert_ne!(r.after, "xyzzy", "after must be a real SHA");
}
