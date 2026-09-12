//! Shared by `module_brief.registry` and `sow_seed.registry_verdict` (identical shape).
//! Verbatim from `lld.rs:428-456`.

use super::module_brief_core::is_non_empty_str;
use super::violation::{v, Violation};
use serde_json::Value;

pub(crate) fn check_registry_verdict(value: Option<&Value>, path_prefix: &str, out: &mut Vec<Violation>) {
    let Some(r) = value.and_then(Value::as_object) else {
        out.push(v(path_prefix, "must be a RegistryVerdict object"));
        return;
    };
    match r.get("kind").and_then(Value::as_str) {
        Some("install") | Some("extract") => {
            if !is_non_empty_str(r.get("matched_path")) {
                out.push(v(format!("{path_prefix}.matched_path"), "matched_path must be a non-empty string"));
            }
        }
        Some("build_new") => {
            let ok = matches!(r.get("searched"), Some(Value::Array(items)) if !items.is_empty() && items.iter().all(|s| matches!(s, Value::String(x) if !x.trim().is_empty())));
            if !ok {
                out.push(v(format!("{path_prefix}.searched"), "searched must be a non-empty array of non-empty strings"));
            }
        }
        _ => out.push(v(format!("{path_prefix}.kind"), "kind must be one of install | extract | build_new")),
    }
}
