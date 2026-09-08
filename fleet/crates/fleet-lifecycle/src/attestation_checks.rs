//! The deep-shape completeness checks for the 3 documented elements, plus
//! `is_valid_artifact_id`. Ported verbatim from
//! `fleet/keel/fleet/src/lifecycle.rs:396-462`.

use serde_json::Value;

pub(crate) fn independent_verification_is_complete(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    object.len() == 5
        && object.get("builder").and_then(Value::as_str).is_some()
        && object.get("verifier").and_then(Value::as_str).is_some()
        && object.get("distinct") == Some(&Value::Bool(true))
        && object.get("reproduced").and_then(Value::as_bool) == Some(true)
        && object.get("verdict").and_then(Value::as_str) == Some("ACCEPT")
}

pub(crate) fn oracle_independence_is_complete(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    object.len() == 6
        && object.get("o1_author").and_then(Value::as_str).is_some()
        && object.get("o2_author").and_then(Value::as_str).is_some()
        && object.get("o1_hash").and_then(Value::as_str).is_some_and(is_valid_artifact_id)
        && object.get("o2_hash").and_then(Value::as_str).is_some_and(is_valid_artifact_id)
        && object.get("distinct") == Some(&Value::Bool(true))
        && object.get("quadrant").and_then(Value::as_str).is_some_and(|quadrant| {
            ["ACCEPT", "ORACLE_INADEQUATE", "ORACLE_OVERCONSTRAINED", "BUILDER_FAULT"]
                .contains(&quadrant)
        })
}

pub(crate) fn blind_suite_is_complete(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    object.len() == 5
        && object.get("in_worktree_tree").and_then(Value::as_bool).is_some()
        && object.get("in_object_store").and_then(Value::as_bool).is_some()
        && object.get("in_env").and_then(Value::as_bool).is_some()
        && object.get("on_any_fd").and_then(Value::as_bool).is_some()
        && object.get("suite_hash").and_then(Value::as_str).is_some()
}

/// Mirrors `main.rs`'s `valid_artifact_id`: two lines of format checking, not a second
/// attestation validator.
pub(crate) fn is_valid_artifact_id(id: &str) -> bool {
    id.len() == 64 && id.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
