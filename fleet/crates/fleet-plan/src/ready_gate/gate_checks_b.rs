//! 4 more of the 14 depth-readiness checks. Verbatim from `lld_ready.rs:385-450`.

use super::gate_text::{as_array, as_str, trim_len};
use super::gate_text_rules::{contains_absolute_claim, valid_revive_trigger};
use super::gate_types::GateRefs;
use serde_json::Value;

pub(crate) fn check_r19_absolute(b: &Value, _refs: &GateRefs) -> bool {
    as_array(b.get("guarantees")).iter().all(|g| {
        let claim = as_str(g.get("claim")).unwrap_or("");
        if !contains_absolute_claim(claim) {
            return true;
        }
        let label_ok = as_str(g.get("label")) == Some("kills_structural");
        let kind_ok = as_str(g.get("derivation").and_then(|d| d.get("kind"))) == Some("structural");
        label_ok && kind_ok
    })
}

pub(crate) fn check_r21_alts(b: &Value, _refs: &GateRefs) -> bool {
    let alts = as_array(b.get("alternatives"));
    if alts.len() < 2 {
        return false;
    }
    let all_fields_present = alts.iter().all(|a| {
        trim_len(a.get("option")) != 0 && trim_len(a.get("why_killed")) != 0 && trim_len(a.get("revive_trigger")) != 0
    });
    if !all_fields_present {
        return false;
    }
    let valid_triggers = alts.iter().all(|a| valid_revive_trigger(as_str(a.get("revive_trigger")).unwrap_or("")));
    if !valid_triggers {
        return false;
    }
    let mut options: Vec<String> = alts.iter().map(|a| as_str(a.get("option")).unwrap_or("").trim().to_lowercase()).collect();
    let total = options.len();
    options.sort();
    options.dedup();
    options.len() == total
}

pub(crate) fn check_r21_fail(b: &Value, _refs: &GateRefs) -> bool {
    let f = b.get("failure_story");
    trim_len(f.and_then(|f| f.get("trigger"))) >= 10
        && trim_len(f.and_then(|f| f.get("blast_radius"))) >= 10
        && trim_len(f.and_then(|f| f.get("fail_safe"))) >= 10
}

pub(crate) fn check_c12_store(b: &Value, _refs: &GateRefs) -> bool {
    let node_id = as_str(b.get("node_id"));
    let items = as_array(b.get("data_owned"));
    let owned_ok = items.iter().all(|d| as_str(d.get("owned_by_node")).is_some() && as_str(d.get("owned_by_node")) == node_id);
    if !owned_ok {
        return false;
    }
    let mut stores: Vec<&str> = items.iter().map(|d| as_str(d.get("store")).unwrap_or("")).collect();
    let total = stores.len();
    stores.sort();
    stores.dedup();
    stores.len() == total
}
