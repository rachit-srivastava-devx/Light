//! Value-level parity: `schema_parity.rs` only checks that field *names* match between the Rust
//! mirrors and the JSON-Schema contracts. That let `DeliveryAttestationV1`'s serialized string
//! drift from `attestation.v1.json`'s `predicateType` const, and let `ReceiptEvent::Rollback`
//! drift out of `receipt.v1.json`'s `event` enum, without either test noticing. These assert the
//! actual values, not just key presence, so that class of drift fails the build again.

#[path = "schema_parity/support.rs"]
#[allow(dead_code)]
mod support;

use fleet_types::ReceiptEvent;
use serde_json::Value;
use support::sample_attestation;

const RECEIPT_SCHEMA: &str = include_str!("../contracts/receipt.v1.json");
const ATTESTATION_SCHEMA: &str = include_str!("../contracts/attestation.v1.json");

#[test]
fn attestation_predicate_type_value_matches_schema_const() {
    let schema: Value = serde_json::from_str(ATTESTATION_SCHEMA).unwrap();
    let instance = serde_json::to_value(sample_attestation()).unwrap();

    let expected = schema["properties"]["predicateType"]["const"]
        .as_str()
        .expect("attestation.v1.json must declare predicateType as a const string");
    let actual = instance["predicateType"]
        .as_str()
        .expect("serialized Attestation must have a string predicateType");

    assert_eq!(actual, expected, "DeliveryAttestationV1 wire value has drifted from the schema");
}

#[test]
fn every_receipt_event_variant_is_in_schema_enum() {
    let schema: Value = serde_json::from_str(RECEIPT_SCHEMA).unwrap();
    let allowed: Vec<&str> = schema["properties"]["event"]["enum"]
        .as_array()
        .expect("receipt.v1.json must declare event as an enum")
        .iter()
        .map(|v| v.as_str().expect("enum entries are strings"))
        .collect();

    let variants = [
        ReceiptEvent::RunStart,
        ReceiptEvent::ArtifactFrozen,
        ReceiptEvent::Attested,
        ReceiptEvent::Refusal,
        ReceiptEvent::GateVerdict,
        ReceiptEvent::RunEnd,
        ReceiptEvent::LaneStatus,
        ReceiptEvent::Rollback,
    ];

    for variant in variants {
        let value = serde_json::to_value(variant).unwrap();
        let value = value.as_str().unwrap();
        assert!(allowed.contains(&value), "ReceiptEvent variant {value:?} missing from schema enum");
    }
}
