//! Byte-faithful port of `validate_sow`'s structural checks (`intake.sh:181-194`) EXCLUDING the
//! hash comparison, which `validate_sow` computes via `hash_file` (`intake.sh:50`, a `shasum`
//! subprocess) -- that hash is a parameter here (`intent_hash`), computed by the caller.

use super::sow_checks::{contains_any_ci, has_line_prefix_ci, has_measurable_threshold, section_body};

/// One SOW/atomic/challenge/clarification structural defect: a human-readable reason, matching
/// the `echo '...' >&2; return 1` shape every `validate_*` fn in `intake.sh` uses today.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{0}")]
pub struct StageViolation(pub String);

const HEADINGS: [&str; 6] = [
    "Request restatement",
    "Built for",
    "Must do",
    "Explicitly will not do",
    "Done when",
    "Acceptance threshold",
];

/// Byte-faithful port of `validate_sow_text`. `intent_hash` stands in for `hash_file`'s subprocess
/// output (`intake.sh:50,183`).
pub fn validate_sow_text(sow_text: &str, intent_hash: &str) -> Vec<StageViolation> {
    let mut out = Vec::new();
    if sow_text.trim().is_empty() {
        out.push(StageViolation("SOW file is missing or empty".into()));
        return out;
    }
    let expected = sow_text
        .lines()
        .find(|l| l.to_lowercase().starts_with("source_intent_hash:"))
        .map(|l| l.split_once(':').map(|x| x.1).unwrap_or("").trim().to_string());
    if expected.as_deref() != Some(intent_hash) {
        out.push(StageViolation(format!(
            "source_intent_hash does not match intent.txt (expected {intent_hash})"
        )));
    }
    for heading in HEADINGS {
        match section_body(sow_text, heading) {
            None => out.push(StageViolation(format!("missing SOW section: {heading}"))),
            Some(body) if body.trim().is_empty() => {
                out.push(StageViolation(format!("SOW section is empty: {heading}")))
            }
            Some(_) => {}
        }
    }
    if !has_line_prefix_ci(sow_text, "request:") {
        out.push(StageViolation("SOW needs a non-empty request: line".into()));
    }
    if !has_measurable_threshold(sow_text) {
        out.push(StageViolation("SOW needs a measurable acceptance threshold".into()));
    }
    if !contains_any_ci(sow_text, &["not", "out of scope", "excluded", "forbidden"]) {
        out.push(StageViolation("SOW needs explicit non-goals".into()));
    }
    if contains_any_ci(
        sow_text,
        &["tbd", "todo", "[fill", "placeholder", "to be decided", "lorem ipsum"],
    ) {
        out.push(StageViolation("SOW contains an unresolved placeholder".into()));
    }
    out
}
