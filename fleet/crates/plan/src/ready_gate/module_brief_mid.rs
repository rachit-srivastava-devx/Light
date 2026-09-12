//! `validate_module_brief`'s interface/data_owned/deps/registry/acceptance checks
//! (`lld.rs:231-315`) -- split out of `module_brief.rs` to stay under the 80-line cap.

use super::module_brief_core::{as_str, is_non_empty_str, is_string_array, str_trim_len};
use super::registry_verdict::check_registry_verdict;
use super::violation::v;
use super::violation::Violation;
use serde_json::{Map, Value};

pub(crate) fn check_module_brief_mid(b: &Map<String, Value>, node_id: Option<&str>, out: &mut Vec<Violation>) {
    match b.get("interface").and_then(Value::as_array) {
        Some(items) if !items.is_empty() => {
            for (i, item) in items.iter().enumerate() {
                let ok = item.as_object().is_some_and(|o| is_non_empty_str(o.get("name")) && is_non_empty_str(o.get("signature")));
                if !ok {
                    out.push(v(format!("interface[{i}]"), "each interface entry needs a non-empty name and signature"));
                }
            }
        }
        _ => out.push(v("interface", "interface must be a non-empty array")),
    }

    match b.get("data_owned").and_then(Value::as_array) {
        Some(items) => {
            for (i, item) in items.iter().enumerate() {
                let obj = item.as_object();
                let shape_ok = obj.is_some_and(|o| is_non_empty_str(o.get("store")) && is_non_empty_str(o.get("owned_by_node")));
                if !shape_ok {
                    out.push(v(format!("data_owned[{i}]"), "each data_owned entry needs a non-empty store and owned_by_node"));
                } else if as_str(obj.and_then(|o| o.get("owned_by_node"))) != node_id {
                    out.push(v(format!("data_owned[{i}].owned_by_node"), "owned_by_node must equal the brief's own node_id"));
                }
            }
        }
        None => out.push(v("data_owned", "data_owned must be an array")),
    }

    if !is_string_array(b.get("deps")) {
        out.push(v("deps", "deps must be an array of strings"));
    }
    check_registry_verdict(b.get("registry"), "registry", out);
    check_acceptance(b.get("acceptance"), out);
}

fn check_acceptance(a: Option<&Value>, out: &mut Vec<Violation>) {
    match a.and_then(Value::as_object) {
        Some(a) => {
            if str_trim_len(a.get("given")) < 3 {
                out.push(v("acceptance.given", "given must be >=3 non-blank characters"));
            }
            if str_trim_len(a.get("when")) < 3 {
                out.push(v("acceptance.when", "when must be >=3 non-blank characters"));
            }
            if str_trim_len(a.get("then")) < 3 {
                out.push(v("acceptance.then", "then must be >=3 non-blank characters"));
            }
            match a.get("oracle_kind").and_then(Value::as_str) {
                Some("test") | Some("property") | Some("metric") => {}
                _ => out.push(v("acceptance.oracle_kind", "oracle_kind must be one of test | property | metric")),
            }
            if !is_non_empty_str(a.get("artifact")) {
                out.push(v("acceptance.artifact", "artifact must be a non-empty string"));
            }
        }
        None => out.push(v("acceptance", "acceptance must be an AcceptanceLine object")),
    }
}
