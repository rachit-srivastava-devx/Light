//! Extra attestation type-marker edge cases, split from `attestation_roundtrip.rs` to keep test
//! files within the crate's 80-line-per-file rule (BLUEPRINT.md §9).

use fleet_types::{
    AttestationElements, BareBlake3Digest, DeliveryAttestationV1, DeliveryTier, InTotoStatementV1,
};

#[test]
fn bare_digest_rejects_prefixed_or_uppercase_hash() {
    assert!(BareBlake3Digest::parse(format!("blake3:{}", "a".repeat(64))).is_err());
    assert!(BareBlake3Digest::parse("A".repeat(64)).is_err());
}

#[test]
fn tier_wire_names_match_schema() {
    assert_eq!(serde_json::to_string(&DeliveryTier::TMin).unwrap(), "\"T-min\"");
    assert_eq!(serde_json::to_string(&DeliveryTier::TStd).unwrap(), "\"T-std\"");
    assert_eq!(serde_json::to_string(&DeliveryTier::TMax).unwrap(), "\"T-max\"");
}

#[test]
fn empty_elements_serializes_to_empty_object() {
    let elements = AttestationElements::default();
    assert_eq!(serde_json::to_string(&elements).unwrap(), "{}");
}

#[test]
fn type_markers_reject_any_other_string() {
    assert!(
        serde_json::from_str::<InTotoStatementV1>("\"https://in-toto.io/Statement/v1\"").is_ok()
    );
    assert!(serde_json::from_str::<InTotoStatementV1>("\"something-else\"").is_err());
    assert!(serde_json::from_str::<DeliveryAttestationV1>("\"something-else\"").is_err());
}
