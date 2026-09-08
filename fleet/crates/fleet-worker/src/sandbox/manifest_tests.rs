use super::*;

#[test]
fn manifest_from_lease_has_expected_shape() {
    let value = manifest_for_lease(".worktrees/lane-1/**").unwrap();
    let tools = value["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 6);
    assert_eq!(value["lease"], ".worktrees/lane-1/**");
}

#[test]
fn traversal_is_rejected() {
    assert!(manifest_for_lease("../escape/**").is_err());
    assert!(manifest_for_lease("/**").is_err());
    assert!(manifest_for_lease("no-suffix").is_err());
}
