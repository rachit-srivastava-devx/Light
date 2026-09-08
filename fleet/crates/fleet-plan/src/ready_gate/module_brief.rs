//! Hand-written mirror of `module-brief.v1.json` (`lld.rs:182-425`, `validate_module_brief`),
//! split across this file (object/field-list/scalar checks) plus `module_brief_mid.rs`
//! (interface/data_owned/deps/registry/acceptance) and `module_brief_nested.rs` (non_goals
//! through failure_story) to stay under the 80-line cap.

use super::module_brief_core::{as_str, is_non_empty_str, valid_node_id};
use super::module_brief_fields::{ALLOWED_MODULE_BRIEF_FIELDS, FORBIDDEN_MODULE_BRIEF_FIELDS};
use super::module_brief_mid::check_module_brief_mid;
use super::module_brief_nested::check_module_brief_rest;
use super::violation::{v, Violation};
use serde_json::Value;

/// Collects every violation (not just the first) so the cross-language comparator can diff
/// error-path SETS across the Rust/TS/Python mirrors.
pub fn validate_module_brief(value: &Value) -> Vec<Violation> {
    let mut out = Vec::new();
    let Some(b) = value.as_object() else {
        out.push(v("", "ModuleBrief must be a JSON object"));
        return out;
    };

    for key in b.keys() {
        if !ALLOWED_MODULE_BRIEF_FIELDS.contains(&key.as_str()) {
            if FORBIDDEN_MODULE_BRIEF_FIELDS.contains(&key.as_str()) {
                out.push(v(key.as_str(), format!("\"{key}\" is gate-authored (Freeze-only) and must not appear on a ModuleBrief")));
            } else {
                out.push(v(key.as_str(), format!("unexpected property \"{key}\" (additionalProperties: false)")));
            }
        }
    }

    if b.get("schema_version").and_then(Value::as_str) != Some("1.0") {
        out.push(v("schema_version", "schema_version must be \"1.0\""));
    }
    let node_id = as_str(b.get("node_id"));
    if !node_id.is_some_and(valid_node_id) {
        out.push(v("node_id", "node_id must match ^[a-z0-9][a-z0-9-]{2,63}$"));
    }
    match b.get("grain").and_then(Value::as_str) {
        Some("module") | Some("leaf") => {}
        _ => out.push(v("grain", "grain must be \"module\" or \"leaf\"")),
    }
    match b.get("purpose").and_then(Value::as_str) {
        Some(p) if !p.is_empty() && p.chars().count() <= 200 => {}
        _ => out.push(v("purpose", "purpose must be a string of 1..200 characters")),
    }
    if !is_non_empty_str(b.get("owner")) {
        out.push(v("owner", "owner must be a non-empty string"));
    }
    match b.get("owner_path").and_then(Value::as_str) {
        Some(p) if !p.trim().is_empty() && !p.contains("..") => {}
        _ => out.push(v("owner_path", "owner_path must be a non-empty path containing no \"..\"")),
    }

    check_module_brief_mid(b, node_id, &mut out);
    check_module_brief_rest(b, &mut out);
    out
}
