//! The pinned `AttestationBundle::REQUIRED` set and `missing_element`'s own behavior.

mod common;

use common::complete_elements;
use fleet_lifecycle::AttestationBundle;
use serde_json::json;

#[test]
fn pinned_element_names_match_the_enforced_set() {
    assert_eq!(
        AttestationBundle::REQUIRED,
        [
            "sow", "blind_suite", "independent_verification", "adequacy", "blast_radius",
            "rollback", "cost", "oracle_independence",
        ]
    );
}

#[test]
fn missing_element_names_the_first_incomplete_or_absent_element() {
    let complete = complete_elements();
    assert_eq!(AttestationBundle::new(complete.clone()).missing_element(), None);

    let mut pending = complete.clone();
    pending["oracle_independence"] = json!({"status": "pending-adjudication"});
    assert_eq!(AttestationBundle::new(pending).missing_element(), Some("oracle_independence"));

    let mut no_adequacy = complete;
    no_adequacy.as_object_mut().unwrap().remove("adequacy");
    assert_eq!(AttestationBundle::new(no_adequacy).missing_element(), Some("adequacy"));
}
