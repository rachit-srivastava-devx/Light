//! The four `__agent` typed-error cases (TASK 3.3), each against the REAL compiled binary, each
//! asserted to exit with its own distinct code -- split out of `agent_child_dispatch.rs` to keep
//! that file ≤80 lines.

mod support;

use support::agent;
use std::fs;

#[test]
fn unknown_adapter_kind_is_a_typed_refusal() {
    let out = agent(&["not-a-real-adapter", "/tmp", "task"]);
    assert!(!out.status.success());
    assert_ne!(out.status.code(), Some(0));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("unknown agent kind"), "stderr: {stderr}");
}

#[test]
fn missing_worktree_is_a_typed_env_failure() {
    let missing = std::env::temp_dir().join(format!("fleet-nope-{}", std::process::id()));
    let _ = fs::remove_dir_all(&missing);
    let out = agent(&["freelane", missing.to_str().unwrap(), "task"]);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("does not exist"), "stderr: {stderr}");
}

#[test]
fn empty_task_is_a_typed_refusal() {
    let out = agent(&["freelane", "/tmp", ""]);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("task is empty"), "stderr: {stderr}");
}

#[test]
fn running_by_hand_without_fd3_gives_a_clear_diagnostic() {
    // Run directly (this test never wires fd 3), exactly how the owner found the original bug.
    let out = agent(&["freelane", "/tmp", "some task"]);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("fd 3 is not open"), "stderr: {stderr}");
}

/// The four cases above must not collapse onto the same exit code -- otherwise a caller (or a
/// test) cannot tell them apart from the outside, which is how this class of bug hides.
#[test]
fn the_four_error_cases_exit_with_four_distinct_codes() {
    let missing = std::env::temp_dir().join(format!("fleet-nope2-{}", std::process::id()));
    let _ = fs::remove_dir_all(&missing);
    let codes: Vec<Option<i32>> = vec![
        agent(&["not-a-real-adapter", "/tmp", "task"]).status.code(),
        agent(&["freelane", missing.to_str().unwrap(), "task"]).status.code(),
        agent(&["freelane", "/tmp", ""]).status.code(),
        agent(&["freelane", "/tmp", "some task"]).status.code(), // fd 3 not open
    ];
    let mut unique = codes.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 4, "expected 4 distinct exit codes, got {codes:?}");
    for code in &codes {
        assert_ne!(*code, Some(0));
        assert_ne!(*code, Some(2), "must never be clap's own usage-error code");
    }
}
