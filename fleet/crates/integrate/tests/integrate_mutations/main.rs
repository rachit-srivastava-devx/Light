use integrate::{cas, Grant, IntegrateError, MergeRequest};
use std::path::PathBuf;

fn make_req(grant: Grant, checked: u64, total: u64) -> MergeRequest {
    MergeRequest {
        repo: PathBuf::from("/tmp"),
        expected_head: "abc".into(),
        lane_head: "lane".into(),
        candidate_digest: "dig".into(),
        grant,
        checked,
        total,
    }
}

fn valid_grant() -> Grant {
    Grant {
        target_ref: "lane".into(),
        candidate_digest: "dig".into(),
        expires_at: 9_999_999_999,
    }
}

#[test]
fn check_grant_target_ref_mismatch_refused() {
    let g = Grant {
        target_ref: "wrong-branch".into(),
        candidate_digest: "dig".into(),
        expires_at: 9_999_999_999,
    };
    let req = make_req(g, 1, 1);
    assert!(matches!(
        cas::check_grant(&req),
        Err(IntegrateError::Grant { .. })
    ));
}

#[test]
fn check_grant_digest_mismatch_refused() {
    let g = Grant {
        target_ref: "lane".into(),
        candidate_digest: "wrong-digest".into(),
        expires_at: 9_999_999_999,
    };
    let req = make_req(g, 1, 1);
    assert!(matches!(
        cas::check_grant(&req),
        Err(IntegrateError::Grant { .. })
    ));
}

#[test]
fn check_grant_zero_checked_refuses() {
    let req = make_req(valid_grant(), 0, 1);
    assert!(matches!(
        cas::check_grant(&req),
        Err(IntegrateError::Grant { .. })
    ));
}

#[test]
fn check_grant_partial_checked_refuses() {
    let req = make_req(valid_grant(), 1, 2);
    assert!(matches!(
        cas::check_grant(&req),
        Err(IntegrateError::Grant { .. })
    ));
}

#[test]
fn check_grant_valid_passes() {
    let req = make_req(valid_grant(), 1, 1);
    assert!(cas::check_grant(&req).is_ok());
}

#[test]
fn path_containment_inside_worktrees_ok() {
    let d = tempfile::tempdir().unwrap();
    let worktrees = d.path().join(".worktrees").join("lane");
    std::fs::create_dir_all(&worktrees).unwrap();
    assert!(cas::path_containment_check(d.path(), &worktrees).is_ok());
}

#[test]
fn path_containment_outside_worktrees_refused() {
    let d = tempfile::tempdir().unwrap();
    let outside = d.path().join("other-dir").join("lane");
    std::fs::create_dir_all(&outside).unwrap();
    assert!(matches!(
        cas::path_containment_check(d.path(), &outside),
        Err(IntegrateError::Containment { .. })
    ));
}
