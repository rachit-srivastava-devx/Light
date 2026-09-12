use fleet_plan::{canonical_json, content_hash, validate_module_brief};
use serde_json::json;

fn minimal_brief() -> serde_json::Value {
    json!({
        "schema_version": "1.0",
        "node_id": "x-y",
        "grain": "module",
        "purpose": "p",
        "owner": "me",
        "owner_path": "a/",
        "interface": [{"name": "f", "signature": "() => void"}],
        "data_owned": [],
        "deps": [],
        "registry": {"kind": "build_new", "searched": ["a"]},
        "acceptance": {"given": "abc", "when": "abc", "then": "abc", "oracle_kind": "test", "artifact": "a"},
        "non_goals": [],
        "open_questions": [],
        "guarantees": [{"claim": "c", "label": "mitigates", "derivation": {"kind": "structural", "invariant": "i", "enforced_by": "gate:x"}}],
        "alternatives": [
            {"option": "a", "why_killed": "w", "revive_trigger": "r"},
            {"option": "b", "why_killed": "w", "revive_trigger": "r"}
        ],
        "failure_story": {"trigger": "0123456789", "blast_radius": "0123456789", "fail_safe": "0123456789"}
    })
}

#[test]
fn validate_module_brief_accepts_a_minimal_well_formed_brief() {
    let violations = validate_module_brief(&minimal_brief());
    assert!(violations.is_empty(), "{violations:?}");
}

#[test]
fn validate_module_brief_rejects_an_extra_top_level_field() {
    let mut brief = minimal_brief();
    brief.as_object_mut().unwrap().insert("depth_evidence".to_string(), json!({"score": {}}));
    let violations = validate_module_brief(&brief);
    assert!(violations.iter().any(|v| v.path == "depth_evidence"), "{violations:?}");
}

#[test]
fn canonical_json_refuses_a_numeric_leaf_at_any_depth() {
    assert!(canonical_json(&json!({"ratio": 1.0})).is_err());
    assert!(canonical_json(&json!({"a": {"b": [1, 2, 3]}})).is_err());
    assert!(canonical_json(&json!({"a": "b", "c": true, "d": null})).is_ok());
}

#[test]
fn canonical_json_sorts_keys_and_is_order_invariant() {
    let a = canonical_json(&json!({"b": "2", "a": "1"})).unwrap();
    let b = canonical_json(&json!({"a": "1", "b": "2"})).unwrap();
    assert_eq!(a, b);
    assert_eq!(a, r#"{"a":"1","b":"2"}"#);
}

#[test]
fn content_hash_matches_the_expected_pattern() {
    let hash = content_hash(&json!({"a": "1"})).unwrap();
    assert!(hash.starts_with("sha256:"));
    assert_eq!(hash.len(), "sha256:".len() + 64);
    assert!(hash["sha256:".len()..].bytes().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
}
