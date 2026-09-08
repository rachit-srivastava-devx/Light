//! Small total accessors + case-insensitive whole-word containment. Every check is total over
//! `serde_json::Value`: a missing, null, or mistyped field makes that check FAIL, never panic.
//! Verbatim from `lld_ready.rs:212-266`.

use serde_json::Value;

pub(crate) fn as_str(v: Option<&Value>) -> Option<&str> {
    match v {
        Some(Value::String(s)) => Some(s.as_str()),
        _ => None,
    }
}

pub(crate) fn trim_len(v: Option<&Value>) -> usize {
    as_str(v).map(|s| s.trim().len()).unwrap_or(0)
}

pub(crate) fn as_array(v: Option<&Value>) -> &[Value] {
    match v {
        Some(Value::Array(items)) => items.as_slice(),
        _ => &[],
    }
}

pub(crate) fn as_str_array(v: Option<&Value>) -> Vec<&str> {
    as_array(v).iter().filter_map(|item| item.as_str()).collect()
}

fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Case-insensitive whole-word/phrase containment, hand-written because this workspace has no
/// `regex` crate. Mirrors a JS `\b...\b` word-boundary match.
pub(crate) fn contains_word_ci(haystack: &str, needle: &str) -> bool {
    let h = haystack.to_lowercase();
    let n = needle.to_lowercase();
    if n.is_empty() {
        return false;
    }
    for (pos, _) in h.match_indices(&n) {
        let before_ok = match h[..pos].chars().next_back() {
            Some(c) => !is_word_char(c),
            None => true,
        };
        let end = pos + n.len();
        let after_ok = match h[end..].chars().next() {
            Some(c) => !is_word_char(c),
            None => true,
        };
        if before_ok && after_ok {
            return true;
        }
    }
    false
}
