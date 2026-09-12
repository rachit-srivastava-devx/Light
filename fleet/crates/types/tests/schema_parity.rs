//! Parity check: the Rust mirrors still match the JSON-Schema contracts they copy.
//!
//! Doc comments elsewhere in this crate used to point at a schema path outside the crate
//! (`fleet`-repo-root `contracts/*.json`), which could drift silently. The schemas now live
//! in-crate (`contracts/*.json`, copied from that root), pulled in with `include_str!` so a
//! missing or renamed fixture is a compile error, not a skipped test. Adding a JSON-Schema
//! validator crate was judged too heavy for two schemas; instead `support::assert_object_matches_schema`
//! walks each schema's `required` list and `additionalProperties` flag by hand against a real
//! serialized instance, at every object level that has one.

#[path = "schema_parity/support.rs"]
mod support;

use serde_json::Value;
use support::{assert_object_matches_schema, sample_attestation, sample_receipt};

const RECEIPT_SCHEMA: &str = include_str!("../contracts/receipt.v1.json");
const ATTESTATION_SCHEMA: &str = include_str!("../contracts/attestation.v1.json");

#[test]
fn receipt_matches_receipt_v1_schema() {
    let schema: Value = serde_json::from_str(RECEIPT_SCHEMA).unwrap();
    let instance = serde_json::to_value(sample_receipt()).unwrap();
    assert_object_matches_schema(&schema, &instance, "Receipt");
}

#[test]
fn attestation_matches_attestation_v1_schema() {
    let schema: Value = serde_json::from_str(ATTESTATION_SCHEMA).unwrap();
    let instance = serde_json::to_value(sample_attestation()).unwrap();
    assert_object_matches_schema(&schema, &instance, "Attestation");

    let subject_schema = &schema["properties"]["subject"]["items"];
    let subject_instance = &instance["subject"][0];
    assert_object_matches_schema(subject_schema, subject_instance, "Attestation.subject[0]");

    let digest_schema = &schema["properties"]["subject"]["items"]["properties"]["digest"];
    let digest_instance = &subject_instance["digest"];
    assert_object_matches_schema(digest_schema, digest_instance, "Attestation.subject[0].digest");

    let predicate_schema = &schema["properties"]["predicate"];
    let predicate_instance = &instance["predicate"];
    assert_object_matches_schema(predicate_schema, predicate_instance, "Attestation.predicate");
}

#[test]
fn schema_files_are_present_and_non_trivial() {
    // include_str! already fails the build if the file is missing; this asserts it isn't
    // an empty stub either, since a build error and a silently-empty fixture fail differently.
    assert!(RECEIPT_SCHEMA.len() > 100, "receipt.v1.json looks truncated");
    assert!(ATTESTATION_SCHEMA.len() > 100, "attestation.v1.json looks truncated");
}
