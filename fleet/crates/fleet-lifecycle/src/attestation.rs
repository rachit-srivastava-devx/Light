//! `AttestationBundle`, `REQUIRED`, `missing_element`. Ported from
//! `fleet/keel/fleet/src/lifecycle.rs:335-394`.

use crate::attestation_checks::{
    blind_suite_is_complete, independent_verification_is_complete,
    oracle_independence_is_complete,
};
use serde_json::Value;

/// The elements of a delivery attestation `propose` must find structurally complete before
/// it asks a `ChangeEmitter` to open a pull request. Built from the attestation's
/// `predicate.elements` object; does not re-derive the caller's own deep validation of it.
#[derive(Clone, Debug)]
pub struct AttestationBundle {
    elements: Value,
}

impl AttestationBundle {
    pub fn new(elements: Value) -> Self {
        Self { elements }
    }

    /// The pinned set of 8 elements `attest_verify_inner` (the caller's validator) enforces
    /// today -- not the aspirational 9th. A test asserts this array matches that enforced set
    /// so a 9th element lands consciously, never silently.
    pub const REQUIRED: [&'static str; 8] = [
        "sow",
        "blind_suite",
        "independent_verification",
        "adequacy",
        "blast_radius",
        "rollback",
        "cost",
        "oracle_independence",
    ];

    /// The first required element missing or structurally incomplete, or `None` if all 8 are
    /// present -- `independent_verification`/`oracle_independence`/`blind_suite` are checked
    /// to their full documented shape; the other five need only be present and non-null.
    pub fn missing_element(&self) -> Option<&'static str> {
        let object = self.elements.as_object();
        for name in Self::REQUIRED {
            let complete = match object.and_then(|map| map.get(name)) {
                None => false,
                Some(value) => match name {
                    "independent_verification" => independent_verification_is_complete(value),
                    "oracle_independence" => oracle_independence_is_complete(value),
                    "blind_suite" => blind_suite_is_complete(value),
                    _ => !value.is_null(),
                },
            };
            if !complete {
                return Some(name);
            }
        }
        None
    }
}
