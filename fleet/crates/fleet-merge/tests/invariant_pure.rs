//! Pure unit tests for the D31 gate -- no git process spawned.

use fleet_merge::{check_files_changed, check_head_moved, check_stage_nonempty, MergeRefusal};
use fleet_types::ExitCode;

#[test]
fn stage_nonempty_rejects_zero_and_accepts_any_positive_count() {
    assert!(matches!(
        check_stage_nonempty(0, "fleet/x"),
        Err(MergeRefusal::EmptyStage { .. })
    ));
    assert!(check_stage_nonempty(1, "fleet/x").is_ok());
    assert!(check_stage_nonempty(usize::MAX, "fleet/x").is_ok());
}

#[test]
fn head_moved_rejects_identical_shas_and_accepts_any_difference() {
    assert!(matches!(
        check_head_moved("abc", "abc", "fleet/x"),
        Err(MergeRefusal::HeadUnmoved { .. })
    ));
    assert!(check_head_moved("abc", "def", "fleet/x").is_ok());
}

#[test]
fn files_changed_rejects_zero_and_accepts_any_positive_count() {
    assert!(matches!(
        check_files_changed(0, "fleet/x"),
        Err(MergeRefusal::NoFilesChanged { .. })
    ));
    assert!(check_files_changed(1, "fleet/x").is_ok());
    assert!(check_files_changed(usize::MAX, "fleet/x").is_ok());
}

#[test]
fn exit_code_mapping_matches_merge_lane_sh() {
    let branch = "fleet/x".to_string();
    let variants = vec![
        MergeRefusal::NoWorktree(std::path::PathBuf::from("/tmp/x")),
        MergeRefusal::StageFailed(std::path::PathBuf::from("/tmp/x")),
        MergeRefusal::EmptyStage { branch: branch.clone() },
        MergeRefusal::CommitFailed { branch: branch.clone() },
        MergeRefusal::Conflict { branch: branch.clone() },
        MergeRefusal::HeadUnmoved { branch: branch.clone() },
        MergeRefusal::NoFilesChanged { branch: branch.clone() },
        MergeRefusal::Spawn("boom".to_string()),
    ];
    for v in variants {
        let expected = match v {
            MergeRefusal::NoWorktree(_) => ExitCode::Env,
            MergeRefusal::StageFailed(_)
            | MergeRefusal::EmptyStage { .. }
            | MergeRefusal::CommitFailed { .. }
            | MergeRefusal::Conflict { .. }
            | MergeRefusal::HeadUnmoved { .. }
            | MergeRefusal::NoFilesChanged { .. }
            | MergeRefusal::Spawn(_) => ExitCode::Invariant,
        };
        assert_eq!(v.exit_code(), expected);
    }
}
