//! The `acceptance` sub-check, split from `module_brief_mid.rs` for size (`lld.rs:231-315`).

use super::super::module_brief_core::{is_non_empty_str, str_trim_len};
use super::super::violation::{v, Violation};
use serde_json::Value;

pub(super) fn check_acceptance(a: Option<&Value>, out: &mut Vec<Violation>) {
    match a.and_then(Value::as_object) {
        Some(a) => {
            if str_trim_len(a.get("given")) < 3 {
                out.push(v(
                    "acceptance.given",
                    "given must be >=3 non-blank characters",
                ));
            }
            if str_trim_len(a.get("when")) < 3 {
                out.push(v(
                    "acceptance.when",
                    "when must be >=3 non-blank characters",
                ));
            }
            if str_trim_len(a.get("then")) < 3 {
                out.push(v(
                    "acceptance.then",
                    "then must be >=3 non-blank characters",
                ));
            }
            match a.get("oracle_kind").and_then(Value::as_str) {
                Some("test") | Some("property") | Some("metric") => {}
                _ => out.push(v(
                    "acceptance.oracle_kind",
                    "oracle_kind must be one of test | property | metric",
                )),
            }
            if !is_non_empty_str(a.get("artifact")) {
                out.push(v(
                    "acceptance.artifact",
                    "artifact must be a non-empty string",
                ));
            }
        }
        None => out.push(v(
            "acceptance",
            "acceptance must be an AcceptanceLine object",
        )),
    }
}
