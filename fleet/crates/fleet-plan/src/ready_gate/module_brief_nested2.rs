//! `validate_module_brief`'s `alternatives`/`failure_story` checks (`lld.rs:383-422`).

use super::module_brief_core::{is_non_empty_str, str_trim_len};
use super::violation::{v, Violation};
use serde_json::{Map, Value};

pub(crate) fn check_alternatives_and_failure_story(b: &Map<String, Value>, out: &mut Vec<Violation>) {
    match b.get("alternatives").and_then(Value::as_array) {
        Some(items) if items.len() >= 2 => {
            for (i, alt) in items.iter().enumerate() {
                let ok = alt.as_object().is_some_and(|o| {
                    is_non_empty_str(o.get("option")) && is_non_empty_str(o.get("why_killed")) && is_non_empty_str(o.get("revive_trigger"))
                });
                if !ok {
                    out.push(v(format!("alternatives[{i}]"), "each alternative needs a non-empty option, why_killed, and revive_trigger"));
                }
            }
        }
        _ => out.push(v("alternatives", "alternatives must have at least 2 entries (flat floor -- no grain exemption, F02 §3.4)")),
    }

    match b.get("failure_story").and_then(Value::as_object) {
        Some(f) => {
            if str_trim_len(f.get("trigger")) < 10 {
                out.push(v("failure_story.trigger", "trigger must be >=10 non-blank characters"));
            }
            if str_trim_len(f.get("blast_radius")) < 10 {
                out.push(v("failure_story.blast_radius", "blast_radius must be >=10 non-blank characters"));
            }
            if str_trim_len(f.get("fail_safe")) < 10 {
                out.push(v("failure_story.fail_safe", "fail_safe must be >=10 non-blank characters"));
            }
        }
        None => out.push(v("failure_story", "failure_story must be a FailureStory object")),
    }
}
