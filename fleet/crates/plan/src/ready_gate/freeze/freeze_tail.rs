//! `check_freeze`'s `depth_evidence`/`supersedes`/`stamped_by` checks (`lld.rs:588-667`).

use super::module_brief_core::valid_freeze_id;
use super::violation::{v, Violation};
use serde_json::{Map, Value};

#[path = "freeze_depth_score.rs"]
mod freeze_depth_score;
use freeze_depth_score::check_depth_evidence;

pub(crate) fn check_freeze_tail(f: &Map<String, Value>, out: &mut Vec<Violation>) {
    match f.get("depth_evidence").and_then(Value::as_object) {
        Some(de) => check_depth_evidence(de, out),
        None => out.push(v(
            "freeze.depth_evidence",
            "depth_evidence must be a DepthEvidence object",
        )),
    }
    match f.get("supersedes") {
        Some(Value::Null) => {}
        Some(Value::String(s)) if valid_freeze_id(s) => {}
        _ => out.push(v(
            "freeze.supersedes",
            "supersedes must be null or match ^fz-[0-9a-f]{16}$",
        )),
    }
    // The one runtime comparison against the gate's const (F02 §4.1).
    let stamped_by = f
        .get("stamped_by")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if stamped_by != "keel:lld-ready" {
        out.push(v(
            "freeze.stamped_by",
            "stamped_by must equal the gate's const (only the gate may write it)",
        ));
    }
}

