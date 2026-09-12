//! The first 6 of the 14 depth-readiness checks. Verbatim from `lld_ready.rs:317-383`.

use super::gate_text::{as_array, as_str, trim_len};
use super::gate_types::GateRefs;
use super::module_brief_core::{number_calc_ok, structural_enforced_by_ok};
use serde_json::Value;

pub(crate) fn check_c1_open(b: &Value, _refs: &GateRefs) -> bool {
    as_array(b.get("open_questions")).is_empty()
}

pub(crate) fn check_c2_owner(b: &Value, refs: &GateRefs) -> bool {
    match as_str(b.get("owner")) {
        Some(owner) => refs.owners.iter().any(|o| o == owner),
        None => false,
    }
}

pub(crate) fn check_c3_acc_parse(b: &Value, _refs: &GateRefs) -> bool {
    let a = b.get("acceptance");
    let oracle_ok = matches!(
        a.and_then(|a| a.get("oracle_kind")).and_then(Value::as_str),
        Some("test") | Some("property") | Some("metric")
    );
    trim_len(a.and_then(|a| a.get("given"))) >= 3
        && trim_len(a.and_then(|a| a.get("when"))) >= 3
        && trim_len(a.and_then(|a| a.get("then"))) >= 3
        && oracle_ok
}

pub(crate) fn check_c3_acc_ground(b: &Value, _refs: &GateRefs) -> bool {
    let a = b.get("acceptance");
    let artifact = as_str(a.and_then(|a| a.get("artifact"))).unwrap_or("");
    let owner_path = as_str(b.get("owner_path")).unwrap_or("");
    let then = as_str(a.and_then(|a| a.get("then"))).unwrap_or("");
    artifact.starts_with(owner_path) && then.contains(artifact)
}

pub(crate) fn check_c3_acc_nontaut(b: &Value, _refs: &GateRefs) -> bool {
    let a = b.get("acceptance");
    let given = as_str(a.and_then(|a| a.get("given"))).unwrap_or("");
    let then = as_str(a.and_then(|a| a.get("then"))).unwrap_or("");
    if then == given {
        return false;
    }
    !super::gate_text_rules::matches_tautology(then)
}

pub(crate) fn check_r17_deriv(b: &Value, _refs: &GateRefs) -> bool {
    let guarantees = as_array(b.get("guarantees"));
    if guarantees.is_empty() {
        return false;
    }
    guarantees.iter().all(|g| {
        let derivation = g.get("derivation");
        match derivation.and_then(|d| d.get("kind")).and_then(Value::as_str) {
            Some("number") => {
                let calc = as_str(derivation.and_then(|d| d.get("calc"))).unwrap_or("");
                number_calc_ok(calc)
            }
            _ => {
                let enforced_by = as_str(derivation.and_then(|d| d.get("enforced_by"))).unwrap_or("");
                structural_enforced_by_ok(enforced_by)
            }
        }
    })
}
