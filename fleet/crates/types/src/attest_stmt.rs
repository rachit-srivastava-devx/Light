//! The full in-toto `Attestation` statement. `contracts/attestation.v1.json:9,22`.

use serde::{Deserialize, Serialize};

use crate::attest_predicate::DeliveryPredicate;
use crate::attest_subject::AttestationSubject;

pub use crate::attest_predicate::AttestationBuilder;

/// The fixed in-toto statement-type marker. Same unrepresentable-by-construction treatment as
/// `Receipt`'s `SchemaV1`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub struct InTotoStatementV1;

impl Serialize for InTotoStatementV1 {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str("https://in-toto.io/Statement/v1")
    }
}

impl<'de> Deserialize<'de> for InTotoStatementV1 {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        if value == "https://in-toto.io/Statement/v1" {
            Ok(InTotoStatementV1)
        } else {
            Err(serde::de::Error::custom(format!("unexpected _type {value:?}")))
        }
    }
}

/// The fixed fleet Delivery predicate-type marker.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub struct DeliveryAttestationV1;

impl Serialize for DeliveryAttestationV1 {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str("https://fleet.local/DeliveryAttestation/v1")
    }
}

impl<'de> Deserialize<'de> for DeliveryAttestationV1 {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        if value == "https://fleet.local/DeliveryAttestation/v1" {
            Ok(DeliveryAttestationV1)
        } else {
            Err(serde::de::Error::custom(format!("unexpected predicateType {value:?}")))
        }
    }
}

/// The full in-toto Statement carrying fleet's Delivery predicate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Attestation {
    #[serde(rename = "_type")]
    pub statement_type: InTotoStatementV1,
    #[serde(rename = "predicateType")]
    pub predicate_type: DeliveryAttestationV1,
    pub subject: Vec<AttestationSubject>,
    pub predicate: DeliveryPredicate,
}
