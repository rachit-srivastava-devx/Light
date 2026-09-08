//! Shared helper + fixtures for `../schema_parity.rs`. Not a test target itself: cargo only
//! auto-discovers files directly under `tests/`, not this subdirectory, so `#[path]` pulls it in.

use fleet_types::{
    Attestation, AttestationBuilder, AttestationDigest, AttestationElements, AttestationSubject,
    BareBlake3Digest, Blake3Hash, DeliveryAttestationV1, DeliveryPredicate, DeliveryTier,
    ExitCode, InTotoStatementV1, PrevHash, Receipt, ReceiptEvent, SchemaV1,
};
use serde_json::{json, Value};

/// Every `required` field name in `schema` must be a key of `instance`. If `schema` declares
/// `"additionalProperties": false`, every key of `instance` must also be a declared property.
pub fn assert_object_matches_schema(schema: &Value, instance: &Value, where_: &str) {
    let instance_obj = instance.as_object().unwrap_or_else(|| {
        panic!("{where_}: expected a JSON object, got {instance:?}");
    });

    if let Some(required) = schema.get("required").and_then(Value::as_array) {
        for field in required {
            let field = field.as_str().expect("required entries are strings");
            assert!(
                instance_obj.contains_key(field),
                "{where_}: schema requires {field:?} but serialized value has {:?}",
                instance_obj.keys().collect::<Vec<_>>(),
            );
        }
    }

    let closed = schema.get("additionalProperties").and_then(Value::as_bool) == Some(false);
    if closed {
        let allowed: Vec<&str> = schema
            .get("properties")
            .and_then(Value::as_object)
            .map(|props| props.keys().map(String::as_str).collect())
            .unwrap_or_default();
        for key in instance_obj.keys() {
            assert!(
                allowed.contains(&key.as_str()),
                "{where_}: serialized key {key:?} is not in schema properties {allowed:?}",
            );
        }
    }
}

pub fn sample_receipt() -> Receipt {
    Receipt {
        schema_version: SchemaV1,
        seq: 0,
        prev_hash: PrevHash::Genesis,
        hash: Blake3Hash::parse(format!("blake3:{}", "a".repeat(64))).unwrap(),
        ts_wall: "2026-09-08T00:00:00Z".to_string(),
        event: ReceiptEvent::RunStart,
        actor: "fleet-worker".to_string(),
        resolved_model: None,
        exit_code: Some(ExitCode::Ok),
        body: json!({}),
    }
}

pub fn sample_attestation() -> Attestation {
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
