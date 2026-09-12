//! `check_freeze`'s `killed_alternatives`/`accepts_when`/`owner` checks (`lld.rs:566-587`) --
//! `depth_evidence`/`supersedes`/`stamped_by` live in `freeze_tail.rs`.

use super::accepts_when::check_accepts_when;
use super::freeze_tail::check_freeze_tail;
use super::module_brief_core::is_non_empty_str;
use super::violation::{v, Violation};
use serde_json::{Map, Value};

pub(crate) fn check_freeze_rest(f: &Map<String, Value>, out: &mut Vec<Violation>) {
    match f.get("killed_alternatives").and_then(Value::as_array) {
        Some(items) if items.len() >= 2 => {
            for (i, alt) in items.iter().enumerate() {
                let ok = alt.as_object().is_some_and(|o| {
                    is_non_empty_str(o.get("option")) && is_non_empty_str(o.get("why_killed")) && is_non_empty_str(o.get("revive_trigger"))
                });
                if !ok {
                    out.push(v(format!("freeze.killed_alternatives[{i}]"), "each killed_alternatives entry needs a non-empty option, why_killed, and revive_trigger"));
                }
            }
        }
        _ => out.push(v("freeze.killed_alternatives", "killed_alternatives must have at least 2 entries")),
    }
    check_accepts_when(f.get("accepts_when"), "freeze.accepts_when", out);
    if !is_non_empty_str(f.get("owner")) {
        out.push(v("freeze.owner", "owner must be a non-empty string"));
    }
    check_freeze_tail(f, out);
}
