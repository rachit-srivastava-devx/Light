//! `check_freeze`'s `depth_evidence`/`supersedes`/`stamped_by` checks (`lld.rs:588-667`).

use super::module_brief_core::{is_string_array, valid_freeze_id};
use super::violation::{v, Violation};
use serde_json::{Map, Value};

pub(crate) fn check_freeze_tail(f: &Map<String, Value>, out: &mut Vec<Violation>) {
    match f.get("depth_evidence").and_then(Value::as_object) {
        Some(de) => check_depth_evidence(de, out),
        None => out.push(v("freeze.depth_evidence", "depth_evidence must be a DepthEvidence object")),
    }
    match f.get("supersedes") {
        Some(Value::Null) => {}
        Some(Value::String(s)) if valid_freeze_id(s) => {}
        _ => out.push(v("freeze.supersedes", "supersedes must be null or match ^fz-[0-9a-f]{16}$")),
    }
    // The one runtime comparison against the gate's const (F02 §4.1).
    let stamped_by = f.get("stamped_by").and_then(Value::as_str).unwrap_or_default();
    if stamped_by != "keel:lld-ready" {
        out.push(v("freeze.stamped_by", "stamped_by must equal the gate's const (only the gate may write it)"));
    }
}

fn check_depth_evidence(de: &Map<String, Value>, out: &mut Vec<Violation>) {
    for key in de.keys() {
        if key != "score" && key != "checked_check_ids" {
            out.push(v(format!("freeze.depth_evidence.{key}"), format!("unexpected property \"{key}\" (additionalProperties: false)")));
        }
    }
    match de.get("score").and_then(Value::as_object) {
        Some(s) => {
            let checks_total_ok = matches!(s.get("checks_total"), Some(Value::Number(n)) if n.as_i64().is_some_and(|x| x >= 0));
            if !checks_total_ok {
                out.push(v("freeze.depth_evidence.score.checks_total", "checks_total must be an integer >= 0"));
            }
            let checks_passed_ok = matches!(s.get("checks_passed"), Some(Value::Number(n)) if n.as_i64().is_some_and(|x| x >= 0));
            if !checks_passed_ok {
                out.push(v("freeze.depth_evidence.score.checks_passed", "checks_passed must be an integer >= 0"));
            }
            let ratio_ok = matches!(s.get("ratio"), Some(Value::Number(n)) if n.as_f64().is_some_and(|x| (0.0..=1.0).contains(&x)));
            if !ratio_ok {
                out.push(v("freeze.depth_evidence.score.ratio", "ratio must be a number in [0,1]"));
            }
            if !is_string_array(s.get("failed_check_ids")) {
                out.push(v("freeze.depth_evidence.score.failed_check_ids", "failed_check_ids must be an array of strings"));
            }
        }
        None => out.push(v("freeze.depth_evidence.score", "score must be a DepthScore object")),
    }
    if !is_string_array(de.get("checked_check_ids")) {
        out.push(v("freeze.depth_evidence.checked_check_ids", "checked_check_ids must be an array of strings"));
    }
}
