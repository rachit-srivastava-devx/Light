//! Shared by `freeze.accepts_when` and `sow_seed.blind_suite_seed` (identical shape). Verbatim
//! from `lld.rs:459-519`.

use super::module_brief_core::{is_non_empty_str, str_trim_len};
use super::violation::{v, Violation};
use serde_json::Value;

pub(crate) fn check_accepts_when(value: Option<&Value>, path_prefix: &str, out: &mut Vec<Violation>) {
    let Some(o) = value.and_then(Value::as_object) else {
        out.push(v(path_prefix, "must be an AcceptsWhen object"));
        return;
    };
    for key in o.keys() {
        if key != "predicate" && key != "artifact" {
            out.push(v(format!("{path_prefix}.{key}"), format!("unexpected property \"{key}\" (additionalProperties: false)")));
        }
    }
    match o.get("predicate").and_then(Value::as_object) {
        Some(p) => {
            for key in p.keys() {
                if !["given", "when", "then", "oracle_kind"].contains(&key.as_str()) {
                    out.push(v(format!("{path_prefix}.predicate.{key}"), format!("unexpected property \"{key}\" (additionalProperties: false)")));
                }
            }
            if str_trim_len(p.get("given")) < 3 {
                out.push(v(format!("{path_prefix}.predicate.given"), "given must be >=3 non-blank characters"));
            }
            if str_trim_len(p.get("when")) < 3 {
                out.push(v(format!("{path_prefix}.predicate.when"), "when must be >=3 non-blank characters"));
            }
            if str_trim_len(p.get("then")) < 3 {
                out.push(v(format!("{path_prefix}.predicate.then"), "then must be >=3 non-blank characters"));
            }
            match p.get("oracle_kind").and_then(Value::as_str) {
                Some("test") | Some("property") | Some("metric") => {}
                _ => out.push(v(format!("{path_prefix}.predicate.oracle_kind"), "oracle_kind must be one of test | property | metric")),
            }
        }
        None => out.push(v(format!("{path_prefix}.predicate"), "predicate must be an object")),
    }
    if !is_non_empty_str(o.get("artifact")) {
        out.push(v(format!("{path_prefix}.artifact"), "artifact must be a non-empty string"));
    }
}
