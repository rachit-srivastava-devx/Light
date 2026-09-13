//! `validate_module_brief`'s non_goals/open_questions/guarantees checks (`lld.rs:317-381`) --
//! split out of `module_brief.rs`; `alternatives`/`failure_story` live in
//! `module_brief_alternatives.rs` to stay under the 80-line cap.

use super::module_brief_core::is_string_array;
use super::module_brief_alternatives::check_alternatives_and_failure_story;
use super::violation::{v, Violation};
use serde_json::{Map, Value};

#[path = "module_brief_guarantees.rs"]
mod module_brief_guarantees;
use module_brief_guarantees::check_one_guarantee;

pub(crate) fn check_module_brief_rest(b: &Map<String, Value>, out: &mut Vec<Violation>) {
    if !is_string_array(b.get("non_goals")) {
        out.push(v("non_goals", "non_goals must be an array of strings"));
    }
    if !is_string_array(b.get("open_questions")) {
        out.push(v(
            "open_questions",
            "open_questions must be an array of strings",
        ));
    }

    match b.get("guarantees").and_then(Value::as_array) {
        Some(items) if !items.is_empty() => {
            for (i, g) in items.iter().enumerate() {
                check_one_guarantee(i, g, out);
            }
        }
        _ => out.push(v("guarantees", "guarantees must be a non-empty array")),
    }

    check_alternatives_and_failure_story(b, out);
}
