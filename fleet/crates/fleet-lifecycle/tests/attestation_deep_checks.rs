//! Structurally-present-but-incomplete `independent_verification`/`blind_suite`/
//! `oracle_independence` elements -- each must still be named missing, not accepted just
//! because the key is non-null (this is the whole point of the deep completeness checks).

mod common;

use common::complete_elements;
use fleet_lifecycle::AttestationBundle;

#[test]
fn independent_verification_with_a_bad_field_is_named_missing() {
    let mut elements = complete_elements();
    elements["independent_verification"]["builder"] = serde_json::json!(123);
    assert_eq!(
        AttestationBundle::new(elements).missing_element(),
        Some("independent_verification")
    );
}

#[test]
fn blind_suite_with_a_bad_field_is_named_missing() {
    let mut elements = complete_elements();
    elements["blind_suite"]["in_worktree_tree"] = serde_json::json!("not-a-bool");
    assert_eq!(AttestationBundle::new(elements).missing_element(), Some("blind_suite"));
}

#[test]
fn oracle_independence_with_a_bad_field_is_named_missing() {
    let mut elements = complete_elements();
    elements["oracle_independence"]["o1_author"] = serde_json::json!(123);
    assert_eq!(AttestationBundle::new(elements).missing_element(), Some("oracle_independence"));
}

#[test]
fn oracle_independence_rejects_a_non_hex_artifact_hash() {
    let mut elements = complete_elements();
    elements["oracle_independence"]["o1_hash"] = serde_json::json!(format!("g{}", "a".repeat(63)));
    assert_eq!(AttestationBundle::new(elements).missing_element(), Some("oracle_independence"));
}

#[test]
fn oracle_independence_rejects_an_uppercase_artifact_hash() {
    let mut elements = complete_elements();
    elements["oracle_independence"]["o1_hash"] = serde_json::json!(format!("A{}", "a".repeat(63)));
    assert_eq!(AttestationBundle::new(elements).missing_element(), Some("oracle_independence"));
}
