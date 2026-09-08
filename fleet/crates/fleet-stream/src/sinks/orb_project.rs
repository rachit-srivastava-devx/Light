//! `OrbSink`'s lane-status projection, split out to keep `orb.rs` under the line cap.

use fleet_types::Receipt;
use serde_json::{json, Value};

use crate::sink::SinkError;

pub(super) fn permanent(seq: u64, reason: impl Into<String>) -> SinkError {
    SinkError::Permanent {
        sink: "orb",
        seq,
        reason: reason.into(),
    }
}

/// Builds the exact `lane-status.v1.json` body: `lane_id`/`role`/`state`/`agent`/
/// `resolved_model` from `body`, `ledger_ref: {seq, hash}` from the receipt envelope. Mirrors
/// `console.rs`'s `load_lane_status` (see §5).
pub(super) fn project(receipt: &Receipt) -> Result<Value, SinkError> {
    let body = receipt
        .body
        .as_object()
        .ok_or_else(|| permanent(receipt.seq, "lane_status body is not an object"))?;
    let lane_id = body
        .get("lane_id")
        .and_then(Value::as_str)
        .ok_or_else(|| permanent(receipt.seq, "lane_status body has no lane_id"))?;
    let role = body.get("role").and_then(Value::as_str).unwrap_or(lane_id);
    let state = body.get("state").and_then(Value::as_str).unwrap_or("unknown");
    Ok(json!({
        "schema_version": "1.0",
        "lane_id": lane_id,
        "role": role,
        "state": state,
        "agent": body.get("agent").cloned().unwrap_or(Value::Null),
        "resolved_model": body.get("resolved_model").cloned().unwrap_or(Value::Null),
        "actor": receipt.actor,
        "ts_wall": receipt.ts_wall,
        "ledger_ref": {"seq": receipt.seq, "hash": receipt.hash.as_str()},
    }))
}
