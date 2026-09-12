//! `check_sow_seed` -- verbatim from `lld.rs:669-699`.

use super::accepts_when::check_accepts_when;
use super::module_brief_core::is_non_empty_str;
use super::module_brief_fields::ALLOWED_SOW_SEED_FIELDS;
use super::registry_verdict::check_registry_verdict;
use super::violation::{v, Violation};
use serde_json::Value;

pub(crate) fn check_sow_seed(value: Option<&Value>, out: &mut Vec<Violation>) {
    let Some(s) = value.and_then(Value::as_object) else {
        out.push(v("sow_seed", "sow_seed must be a SowSeed object"));
        return;
    };
    for key in s.keys() {
        if !ALLOWED_SOW_SEED_FIELDS.contains(&key.as_str()) {
            out.push(v(format!("sow_seed.{key}"), format!("unexpected property \"{key}\" (additionalProperties: false)")));
        }
    }
    if !is_non_empty_str(s.get("restatement")) {
        out.push(v("sow_seed.restatement", "restatement must be a non-empty string"));
    }
    check_accepts_when(s.get("blind_suite_seed"), "sow_seed.blind_suite_seed", out);
    if !is_non_empty_str(s.get("blast_radius")) {
        out.push(v("sow_seed.blast_radius", "blast_radius must be a non-empty string"));
    }
    if !is_non_empty_str(s.get("owner")) {
        out.push(v("sow_seed.owner", "owner must be a non-empty string"));
    }
    check_registry_verdict(s.get("registry_verdict"), "sow_seed.registry_verdict", out);
}
