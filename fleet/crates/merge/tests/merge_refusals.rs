//! `merge_lane` refusal-path integration tests against real `tempfile::TempDir` repos.

mod common;

use common::{init_repo, rev_parse_head, run};
use fleet_merge::{create, merge_lane, remove, MergeRefusal};

#[test]
fn merge_lane_refuses_when_worktree_is_untouched() {
    let (_guard, repo) = init_repo();
    let name = fleet_merge::unique_name("untouched");
    let wt = create(&repo, &name).expect("create");
    let before = rev_parse_head(&repo);

    let err = merge_lane(&repo, &wt.path, &wt.branch).expect_err("nothing staged must refuse");
    assert!(matches!(err, MergeRefusal::EmptyStage { .. }));
    assert_eq!(before, rev_parse_head(&repo));

    remove(&repo, &wt).expect("cleanup");
}

#[test]
fn merge_lane_refuses_on_repeat_merge_of_an_already_merged_branch() {
    // Degenerate-but-real repro of "already up to date": commit directly on the repo's own
    // checked-out branch, then ask merge_lane to merge that same branch into the same repo.
    // `before` is captured only after the commit (matching `merge_lane`'s own order), so the
    // merge step -- the same branch already at repo's HEAD -- is a true git no-op.
    let (_guard, repo) = init_repo();
    let branch = {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(["branch", "--show-current"])
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    };
    std::fs::write(repo.join("new.txt"), b"n").unwrap();

    let err = merge_lane(&repo, &repo, &branch).expect_err("self-merge must refuse");
    assert!(matches!(err, MergeRefusal::HeadUnmoved { .. }));
}

#[test]
fn merge_lane_reports_conflict_without_corrupting_the_target_repo() {
    let (_guard, repo) = init_repo();
    let name = fleet_merge::unique_name("conflict");
    let wt = create(&repo, &name).expect("create");

    std::fs::write(repo.join("f.txt"), b"y").unwrap();
    run(&repo, &["add", "."]);
    run(&repo, &["commit", "-q", "-m", "diverge on repo side"]);
    let before = rev_parse_head(&repo);

    std::fs::write(wt.path.join("f.txt"), b"z").unwrap();
    let err = merge_lane(&repo, &wt.path, &wt.branch).expect_err("conflict must refuse");
    assert!(matches!(err, MergeRefusal::Conflict { .. }));
    assert_eq!(before, rev_parse_head(&repo));

    run(&repo, &["merge", "--abort"]);
    remove(&repo, &wt).expect("cleanup");
}
