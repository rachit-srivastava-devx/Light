//! Hand-written mirror of `lld.v1.json`, the wrapper crossing the orb->fleet seam. Verbatim from
//! `lld.rs:701-755`, delegating to `validate_module_brief` for the nested `module_brief` object.

use super::freeze::check_freeze;
use super::module_brief::validate_module_brief;
use super::module_brief_core::as_str;
use super::module_brief_fields::ALLOWED_LLD_V1_FIELDS;
use super::sow_seed::check_sow_seed;
use super::violation::{v, Violation};
use serde_json::Value;

pub fn validate_lld_v1(value: &Value) -> Vec<Violation> {
    let mut out = Vec::new();
    let Some(top) = value.as_object() else {
        out.push(v("", "lld.v1 must be a JSON object"));
        return out;
    };

    for key in top.keys() {
        if !ALLOWED_LLD_V1_FIELDS.contains(&key.as_str()) {
            out.push(v(key.as_str(), format!("unexpected property \"{key}\" (additionalProperties: false)")));
        }
    }
    if top.get("schema_version").and_then(Value::as_str) != Some("1.0") {
        out.push(v("schema_version", "schema_version must be \"1.0\""));
    }

    let module_brief = top.get("module_brief").cloned().unwrap_or(Value::Null);
    for e in validate_module_brief(&module_brief) {
        let path = if e.path.is_empty() { "module_brief".to_string() } else { format!("module_brief.{}", e.path) };
        out.push(v(path, e.message));
    }

    check_freeze(top.get("freeze"), &mut out);
    check_sow_seed(top.get("sow_seed"), &mut out);

    if let (Some(brief_obj), Some(freeze_obj)) =
        (top.get("module_brief").and_then(Value::as_object), top.get("freeze").and_then(Value::as_object))
    {
        if let (Some(bn), Some(fn_)) = (as_str(brief_obj.get("node_id")), as_str(freeze_obj.get("node_id"))) {
            if bn != fn_ {
                out.push(v("freeze.node_id", "freeze.node_id must equal the sibling module_brief.node_id"));
            }
        }
    }

    out
}
