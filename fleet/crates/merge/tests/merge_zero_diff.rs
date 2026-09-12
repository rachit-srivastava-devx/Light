//! Mutation target: `check_files_changed` must fire even when `HEAD` genuinely moves. A merge
//! commit whose two parents both already carry the identical resulting tree moves `HEAD` to a
//! brand-new commit object while the pre/post diff is empty.

mod common;

use common::{init_repo, run};
use fleet_merge::{create, merge_lane, remove, MergeRefusal};

#[test]
fn merge_lane_refuses_on_head_move_with_zero_file_diff() {
    let (_guard, repo) = init_repo();
    let name = fleet_merge::unique_name("zerodiff");
    let wt = create(&repo, &name).expect("create");

    // Repo side: f.txt "x" -> "y", committed directly on repo's own branch.
    std::fs::write(repo.join("f.txt"), b"y").unwrap();
    run(&repo, &["add", "."]);
    run(&repo, &["commit", "-q", "-m", "repo side moves to y"]);

    // Lane side: same edit, "x" -> "y", staged via merge_lane's own commit step.
    std::fs::write(wt.path.join("f.txt"), b"y").unwrap();
    let outcome = merge_lane(&repo, &wt.path, &wt.branch);
    match outcome {
        Err(MergeRefusal::NoFilesChanged { .. }) => {}
        other => panic!("expected NoFilesChanged, got {other:?}"),
    }

    remove(&repo, &wt).expect("cleanup");
}
