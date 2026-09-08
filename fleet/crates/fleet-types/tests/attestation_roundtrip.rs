//! Serde round-trip against `attestation.v1.json`'s shape (BLUEPRINT.md §9).

use fleet_types::{
    Attestation, AttestationBuilder, AttestationDigest, AttestationElements, AttestationSubject,
    BareBlake3Digest, DeliveryAttestationV1, DeliveryPredicate, DeliveryTier, InTotoStatementV1,
};
use serde_json::json;

fn sample_attestation() -> Attestation {
    Attestation {
        statement_type: InTotoStatementV1,
        predicate_type: DeliveryAttestationV1,
        subject: vec![AttestationSubject {
            name: "diff.patch".to_string(),
            digest: AttestationDigest {
                blake3: BareBlake3Digest::parse("b".repeat(64)).unwrap(),
            },
        }],
        predicate: DeliveryPredicate {
            tier: DeliveryTier::TStd,
            builder: AttestationBuilder { id: "fleet-worker".to_string() },
            elements: AttestationElements {
                oracle_independence: Some(json!({"checked": true})),
                ..Default::default()
            },
            receipts: vec!["blake3:aaaa".to_string()],
        },
    }
}

#[test]
fn attestation_round_trips_byte_for_byte() {
    let attestation = sample_attestation();
    let serialized = serde_json::to_string(&attestation).unwrap();
    let deserialized: Attestation = serde_json::from_str(&serialized).unwrap();
    assert_eq!(attestation, deserialized);
}

#[test]
fn empty_elements_omits_all_eight_keys() {
    let mut attestation = sample_attestation();
    attestation.predicate.elements = AttestationElements::default();
    let serialized = serde_json::to_string(&attestation).unwrap();
    assert!(serialized.contains("\"elements\":{}"));
}

#[test]
fn wrong_predicate_type_fails_to_deserialize() {
    let attestation = sample_attestation();
    let serialized = serde_json::to_string(&attestation).unwrap();
    let bad = serialized.replace(
        "https://fleet.local/DeliveryAttestation/v1",
        "https://example.com/other",
    );
    assert!(serde_json::from_str::<Attestation>(&bad).is_err());
}

