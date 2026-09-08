//! Integration coverage for TaskId/NodeId/LaneId parse edge cases (BLUEPRINT.md §9).

use fleet_types::{valid_artifact_id, LaneId, NodeId, TaskId};

#[test]
fn task_id_and_lane_id_reject_empty_and_whitespace() {
    for bad in ["", "   ", "\t\n"] {
        assert!(TaskId::parse(bad).is_err());
        assert!(LaneId::parse(bad).is_err());
    }
    assert!(TaskId::parse("valid-id").is_ok());
    assert!(LaneId::parse("builder").is_ok());
}

#[test]
fn node_id_matches_module_brief_pattern() {
    assert!(NodeId::parse("abc").is_ok());
    assert!(NodeId::parse("a".repeat(64)).is_ok());
    assert!(NodeId::parse("a".repeat(65)).is_err()); // too long
    assert!(NodeId::parse("ab").is_err()); // too short
    assert!(NodeId::parse("-abc").is_err()); // leading hyphen
    assert!(NodeId::parse("Abc").is_err()); // uppercase
}

#[test]
fn artifact_id_predicate_matches_main_rs_shape() {
    assert!(valid_artifact_id(&"0".repeat(64)));
    assert!(!valid_artifact_id(&"0".repeat(65)));
    assert!(!valid_artifact_id("not-hex-at-all-not-hex-at-all-not-hex-at-all-not-hex-at-all12"));
}
