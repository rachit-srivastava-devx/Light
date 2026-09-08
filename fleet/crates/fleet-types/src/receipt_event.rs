//! `ReceiptEvent`, `SchemaV1`, and the `Receipt` wire record. `contracts/receipt.v1.json`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::exit_code::ExitCode;
use crate::prev_hash::PrevHash;
use crate::receipt::Blake3Hash;

/// `receipt.v1.json`'s `event` enum. Includes `Rollback`, the 8th variant surfaced by
/// MIGRATION-PLAN §7 (fleet's whitelist `main.rs:4373-4386` stores `"rollback"`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptEvent {
    RunStart,
    ArtifactFrozen,
    Attested,
    Refusal,
    GateVerdict,
    RunEnd,
    LaneStatus,
    Rollback,
}

/// The fixed `"1.0"` schema-version marker for `Receipt`. A `Receipt` cannot be constructed
/// carrying any other schema version.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub struct SchemaV1;

impl Serialize for SchemaV1 {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str("1.0")
    }
}

impl<'de> Deserialize<'de> for SchemaV1 {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        if value == "1.0" {
            Ok(SchemaV1)
        } else {
            Err(serde::de::Error::custom(format!(
                "schema_version must be \"1.0\", got {value:?}"
            )))
        }
    }
}

/// One append-only ledger row, matching `contracts/receipt.v1.json` field-for-field.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Receipt {
    pub schema_version: SchemaV1,
    pub seq: u64,
    pub prev_hash: PrevHash,
    pub hash: Blake3Hash,
    /// STAMPED BY THE LEDGER -- never constructed by a worker.
    pub ts_wall: String,
    pub event: ReceiptEvent,
    /// STAMPED BY THE LAUNCHER -- never declared by the launched process.
    pub actor: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub resolved_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub exit_code: Option<ExitCode>,
    pub body: Value,
}
