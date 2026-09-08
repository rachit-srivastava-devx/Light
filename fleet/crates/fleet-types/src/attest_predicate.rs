//! `DeliveryPredicate` and its `elements`. `contracts/attestation.v1.json:23-47`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::attest_subject::DeliveryTier;

/// `attestation.v1.json`'s `predicate.builder`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AttestationBuilder {
    pub id: String,
}

/// `attestation.v1.json`'s `predicate.elements`. Every field is `Option<Value>` since the schema
/// declares no required keys under `elements`; only `oracle_independence` is non-droppable by
/// convention (a doc comment, not a type-level requirement -- the schema itself doesn't enforce
/// it either).
#[derive(Clone, Debug, Eq, PartialEq, Default, Serialize, Deserialize)]
pub struct AttestationElements {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub sow: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub blind_suite: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub independent_verification: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub adequacy: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub blast_radius: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub rollback: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub cost: Option<Value>,
    /// element 8 -- NON-DROPPABLE AT EVERY TIER.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub oracle_independence: Option<Value>,
}

/// `attestation.v1.json`'s `predicate` object.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DeliveryPredicate {
    pub tier: DeliveryTier,
    pub builder: AttestationBuilder,
    #[serde(default)]
    pub elements: AttestationElements,
    pub receipts: Vec<String>,
}
