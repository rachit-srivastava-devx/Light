//! Happy-path `merge_lane` + `invalidate_build_cache` integration tests.

mod common;

use common::init_repo;
use fleet_merge::{create, invalidate_build_cache, merge_lane, remove};

#[test]
fn merge_lane_happy_path_reports_real_staged_and_changed_counts() {
    let (_guard, repo) = init_repo();
    let name = fleet_merge::unique_name("happy");
    let wt = create(&repo, &name).expect("create");
    std::fs::write(wt.path.join("a.txt"), b"a").unwrap();
    std::fs::write(wt.path.join("b.txt"), b"b").unwrap();

    let outcome = merge_lane(&repo, &wt.path, &wt.branch).expect("merge_lane");
    assert_eq!(outcome.staged_files, 2);
    assert_eq!(outcome.changed_files, 2);
    assert_ne!(outcome.before, outcome.after);
    assert!(repo.join("a.txt").exists());
    assert!(repo.join("b.txt").exists());

    remove(&repo, &wt).expect("cleanup");
}

#[test]
fn invalidate_build_cache_never_panics_without_a_manifest() {
    let missing = std::path::Path::new("/definitely/not/a/real/path/Cargo.toml");
    assert!(!invalidate_build_cache(missing, "fleet"));
}
