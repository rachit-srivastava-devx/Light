//! The per-`guarantees[i]` check, split from `module_brief_nested.rs` for size
//! (`lld.rs:317-381`).

use super::super::module_brief_core::{as_str, is_non_empty_str, number_calc_ok, structural_enforced_by_ok};
use super::super::violation::{v, Violation};
use serde_json::Value;

pub(super) fn check_one_guarantee(i: usize, g: &Value, out: &mut Vec<Violation>) {
    let obj = g.as_object();
    if !obj.is_some_and(|o| is_non_empty_str(o.get("claim"))) {
        out.push(v(
            format!("guarantees[{i}].claim"),
            "claim must be a non-empty string",
        ));
    }
    let label_ok = matches!(
        obj.and_then(|o| o.get("label")).and_then(Value::as_str),
        Some("kills_structural") | Some("kills_mechanical") | Some("mitigates")
    );
    if !label_ok {
        out.push(v(
            format!("guarantees[{i}].label"),
            "label must be one of kills_structural | kills_mechanical | mitigates",
        ));
    }
    let derivation = obj
        .and_then(|o| o.get("derivation"))
        .and_then(Value::as_object);
    match derivation {
        None => out.push(v(
            format!("guarantees[{i}].derivation"),
            "derivation must be a Derivation object",
        )),
        Some(d) => {
            match d.get("kind").and_then(Value::as_str) {
                Some("number") => {
                    let calc_ok = as_str(d.get("calc")).is_some_and(number_calc_ok);
                    if !calc_ok {
                        out.push(v(format!("guarantees[{i}].derivation.calc"), "a numeric derivation needs non-empty arithmetic (a digit and an operator)"));
                    }
                }
                Some("structural") => {
                    let enforced_ok =
                        as_str(d.get("enforced_by")).is_some_and(structural_enforced_by_ok);
                    if !enforced_ok {
                        out.push(v(
                            format!("guarantees[{i}].derivation.enforced_by"),
                            "a structural derivation must name a file, type:, or gate:",
                        ));
                    }
                }
                _ => out.push(v(
                    format!("guarantees[{i}].derivation.kind"),
                    "kind must be \"number\" or \"structural\"",
                )),
            }
        }
    }
}
