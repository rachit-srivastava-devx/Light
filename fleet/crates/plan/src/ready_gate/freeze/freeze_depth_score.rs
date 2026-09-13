//! `check_freeze_tail`'s `depth_evidence` sub-check, split out for size (`lld.rs:588-667`).

use super::super::module_brief_core::is_string_array;
use super::super::violation::{v, Violation};
use serde_json::{Map, Value};

pub(super) fn check_depth_evidence(de: &Map<String, Value>, out: &mut Vec<Violation>) {
    for key in de.keys() {
        if key != "score" && key != "checked_check_ids" {
            out.push(v(
                format!("freeze.depth_evidence.{key}"),
                format!("unexpected property \"{key}\" (additionalProperties: false)"),
            ));
        }
    }
    match de.get("score").and_then(Value::as_object) {
        Some(s) => {
            let checks_total_ok = matches!(s.get("checks_total"), Some(Value::Number(n)) if n.as_i64().is_some_and(|x| x >= 0));
            if !checks_total_ok {
                out.push(v(
                    "freeze.depth_evidence.score.checks_total",
                    "checks_total must be an integer >= 0",
                ));
            }
            let checks_passed_ok = matches!(s.get("checks_passed"), Some(Value::Number(n)) if n.as_i64().is_some_and(|x| x >= 0));
            if !checks_passed_ok {
                out.push(v(
                    "freeze.depth_evidence.score.checks_passed",
                    "checks_passed must be an integer >= 0",
                ));
            }
            let ratio_ok = matches!(s.get("ratio"), Some(Value::Number(n)) if n.as_f64().is_some_and(|x| (0.0..=1.0).contains(&x)));
            if !ratio_ok {
                out.push(v(
                    "freeze.depth_evidence.score.ratio",
                    "ratio must be a number in [0,1]",
                ));
            }
            if !is_string_array(s.get("failed_check_ids")) {
                out.push(v(
                    "freeze.depth_evidence.score.failed_check_ids",
                    "failed_check_ids must be an array of strings",
                ));
            }
        }
        None => out.push(v(
            "freeze.depth_evidence.score",
            "score must be a DepthScore object",
        )),
    }
    if !is_string_array(de.get("checked_check_ids")) {
        out.push(v(
            "freeze.depth_evidence.checked_check_ids",
            "checked_check_ids must be an array of strings",
        ));
    }
}
