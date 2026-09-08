//! The canonical byte form a receipt (minus its own `hash`) hashes over. Shared by `append`
//! (which computes a fresh hash) and `verify` (which recomputes one to compare).

use fleet_types::{ExitCode, PrevHash, Receipt, ReceiptEvent, SchemaV1};
use serde_json::{json, Map, Value};

/// Recompute a receipt's content hash from its own fields (schema/seq/prev_hash/ts/event/actor/
/// resolved_model/exit_code/body), the same formula `append` uses to mint a hash and `verify`
/// uses to check one. Needs only the single row -- no chain context.
pub(super) fn recompute_hash(receipt: &Receipt) -> String {
    let canonical = canonical_bytes(
        &receipt.schema_version,
        receipt.seq,
        &receipt.prev_hash,
        &receipt.ts_wall,
        &receipt.event,
        &receipt.actor,
        &receipt.resolved_model,
        &receipt.exit_code,
        &receipt.body,
    );
    let mut input = receipt.prev_hash.as_str().as_bytes().to_vec();
    input.extend_from_slice(&canonical);
    format!("blake3:{}", blake3::hash(&input).to_hex())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn canonical_bytes(
    schema_version: &SchemaV1,
    seq: u64,
    prev_hash: &PrevHash,
    ts_wall: &str,
    event: &ReceiptEvent,
    actor: &str,
    resolved_model: &Option<String>,
    exit_code: &Option<ExitCode>,
    body: &Value,
) -> Vec<u8> {
    let mut map = Map::new();
    map.insert(
        "schema_version".into(),
        serde_json::to_value(schema_version).expect("SchemaV1 always serializes"),
    );
    map.insert("seq".into(), json!(seq));
    map.insert("prev_hash".into(), json!(prev_hash.as_str()));
    map.insert("ts_wall".into(), json!(ts_wall));
    map.insert(
        "event".into(),
        serde_json::to_value(event).expect("ReceiptEvent always serializes"),
    );
    map.insert("actor".into(), json!(actor));
    map.insert(
        "resolved_model".into(),
        resolved_model.clone().map_or(Value::Null, Value::String),
    );
    map.insert(
        "exit_code".into(),
        exit_code
            .as_ref()
            .map_or(Value::Null, |c| serde_json::to_value(c).expect("ExitCode always serializes")),
    );
    map.insert("body".into(), body.clone());
    serde_json::to_vec(&Value::Object(map)).expect("a map of pure JSON values always serializes")
}
