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
fn task_id_error_names_rule_and_shows_input() {
    let err = TaskId::parse("").unwrap_err();
    assert_eq!(err.field, "task_id");
    assert_eq!(err.type_name, "TaskId");
    let msg = err.to_string();
    assert!(
        msg.contains("task_id must not be empty or whitespace-only"),
        "message should name the rule that failed: {msg}"
    );
    assert!(msg.contains("\"\""), "message should quote the offending input: {msg}");
    assert!(
        msg.contains("fleet_types::TaskId::parse"),
        "message should point at the grammar source: {msg}"
    );
}

#[test]
fn task_id_error_shows_whitespace_input_verbatim() {
    let err = TaskId::parse("   ").unwrap_err();
    let msg = err.to_string();
    // Debug-quoted, so trailing whitespace is visible to the user.
    assert!(msg.contains("\"   \""), "whitespace input must be visible in the message: {msg}");
}

#[test]
fn lane_id_error_names_its_own_type() {
    let err = LaneId::parse("\t\n").unwrap_err();
    assert_eq!(err.field, "lane_id");
    assert_eq!(err.type_name, "LaneId");
    let msg = err.to_string();
    assert!(msg.contains("lane_id must not be empty or whitespace-only"), "{msg}");
    assert!(msg.contains("fleet_types::LaneId::parse"), "{msg}");
}

#[test]
fn task_id_error_truncates_very_long_input() {
    // Empty-after-trim input longer than the 60-char preview cap: leading tab
    // then 100 spaces. The tab makes it non-trivial to distinguish from ""
    // without a preview, and the length crosses the truncation threshold.
    let long = format!("\t{}", " ".repeat(100));
    let err = TaskId::parse(long.clone()).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("truncated") && msg.contains(&format!("{} chars total", long.chars().count())),
        "long inputs should be truncated with a length marker: {msg}"
    );
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
