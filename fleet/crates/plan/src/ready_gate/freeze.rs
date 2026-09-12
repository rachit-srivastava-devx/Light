//! `check_freeze` -- field-list + scalar checks (`lld.rs:521-565`). `killed_alternatives` through
//! `stamped_by` live in `freeze_depth_evidence.rs` to stay under the 80-line cap.

use super::freeze_depth_evidence::check_freeze_rest;
use super::module_brief_core::{as_str, is_non_empty_str, valid_content_hash, valid_freeze_id, valid_node_id};
use super::module_brief_fields::ALLOWED_FREEZE_FIELDS;
use super::violation::{v, Violation};
use serde_json::Value;

pub(crate) fn check_freeze(value: Option<&Value>, out: &mut Vec<Violation>) {
    let Some(f) = value.and_then(Value::as_object) else {
        out.push(v("freeze", "freeze must be a Freeze object"));
        return;
    };
    for key in f.keys() {
        if !ALLOWED_FREEZE_FIELDS.contains(&key.as_str()) {
            out.push(v(format!("freeze.{key}"), format!("unexpected property \"{key}\" (additionalProperties: false)")));
        }
    }
    if f.get("schema_version").and_then(Value::as_str) != Some("1.0") {
        out.push(v("freeze.schema_version", "schema_version must be \"1.0\""));
    }
    if !as_str(f.get("freeze_id")).is_some_and(valid_freeze_id) {
        out.push(v("freeze.freeze_id", "freeze_id must match ^fz-[0-9a-f]{16}$"));
    }
    let version_ok = matches!(f.get("version"), Some(Value::Number(n)) if n.as_i64().is_some_and(|x| x >= 1));
    if !version_ok {
        out.push(v("freeze.version", "version must be an integer >= 1"));
    }
    if !as_str(f.get("node_id")).is_some_and(valid_node_id) {
        out.push(v("freeze.node_id", "node_id must match ^[a-z0-9][a-z0-9-]{2,63}$"));
    }
    if !as_str(f.get("content_hash")).is_some_and(valid_content_hash) {
        out.push(v("freeze.content_hash", "content_hash must match ^sha256:[0-9a-f]{64}$"));
    }
    if !is_non_empty_str(f.get("decision")) {
        out.push(v("freeze.decision", "decision must be a non-empty string"));
    }
    if !is_non_empty_str(f.get("why")) {
        out.push(v("freeze.why", "why must be a non-empty string"));
    }
    check_freeze_rest(f, out);
}
