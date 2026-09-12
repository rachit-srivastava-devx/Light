//! The last 4 of the 14 depth-readiness checks. Verbatim from `lld_ready.rs:452-511`.

use super::gate_text::{as_array, as_str, as_str_array};
use super::gate_types::GateRefs;
use super::module_brief_core::valid_node_id;
use serde_json::Value;

pub(crate) fn check_c12_deps(b: &Value, refs: &GateRefs) -> bool {
    let node_id = as_str(b.get("node_id")).unwrap_or("");
    let deps = as_str_array(b.get("deps"));
    if deps.contains(&node_id) {
        return false;
    }
    if !deps.iter().all(|d| refs.known_node_ids.iter().any(|k| k == d)) {
        return false;
    }
    let stores: Vec<&str> = as_array(b.get("data_owned")).iter().map(|item| as_str(item.get("store")).unwrap_or("")).collect();
    !deps.iter().any(|d| stores.contains(d))
}

pub(crate) fn check_reg_verdict(b: &Value, refs: &GateRefs) -> bool {
    let r = b.get("registry");
    match as_str(r.and_then(|r| r.get("kind"))) {
        Some("install") | Some("extract") => {
            let matched = as_str(r.and_then(|r| r.get("matched_path"))).unwrap_or("");
            refs.registry_paths.iter().any(|p| p == matched)
        }
        _ => {
            let searched = as_str_array(r.and_then(|r| r.get("searched")));
            !searched.is_empty() && searched.iter().all(|p| refs.registry_paths.iter().any(|rp| rp == p))
        }
    }
}

pub(crate) fn check_iface(b: &Value, _refs: &GateRefs) -> bool {
    let items = as_array(b.get("interface"));
    if items.is_empty() {
        return false;
    }
    items.iter().all(|decl| {
        let sig = as_str(decl.get("signature")).unwrap_or("");
        (sig.contains('(') && sig.contains(')')) || sig.starts_with("type ") || sig.starts_with("interface ")
    })
}

pub(crate) fn check_shape(b: &Value, _refs: &GateRefs) -> bool {
    let node_id = as_str(b.get("node_id")).unwrap_or("");
    let purpose = as_str(b.get("purpose")).unwrap_or("");
    let owner_path = as_str(b.get("owner_path")).unwrap_or("");
    valid_node_id(node_id) && !purpose.is_empty() && purpose.chars().count() <= 200 && !owner_path.is_empty() && !owner_path.contains("..")
}
