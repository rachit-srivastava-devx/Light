//! Small total accessors + the three hand-checked `^...$` patterns (node_id/freeze_id/
//! content_hash) plus the numeric-derivation/structural-enforcement predicates. Verbatim from
//! `lld.rs:37-127`. No `regex` crate anywhere in this workspace (`lld.rs:59-63`'s convention).

use serde_json::Value;

pub(crate) fn is_non_empty_str(x: Option<&Value>) -> bool {
    matches!(x, Some(Value::String(s)) if !s.trim().is_empty())
}

pub(crate) fn str_trim_len(x: Option<&Value>) -> usize {
    match x {
        Some(Value::String(s)) => s.trim().len(),
        _ => 0,
    }
}

pub(crate) fn as_str(x: Option<&Value>) -> Option<&str> {
    match x {
        Some(Value::String(s)) => Some(s.as_str()),
        _ => None,
    }
}

pub(crate) fn is_string_array(x: Option<&Value>) -> bool {
    matches!(x, Some(Value::Array(items)) if items.iter().all(|i| matches!(i, Value::String(_))))
}

/// `^[a-z0-9][a-z0-9-]{2,63}$`.
pub(crate) fn valid_node_id(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() < 3 || b.len() > 64 {
        return false;
    }
    let head_ok = b[0].is_ascii_lowercase() || b[0].is_ascii_digit();
    head_ok && b[1..].iter().all(|&c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
}

/// `^fz-[0-9a-f]{16}$`.
pub(crate) fn valid_freeze_id(s: &str) -> bool {
    s.len() == 19 && s.starts_with("fz-") && s[3..].bytes().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

/// `^sha256:[0-9a-f]{64}$`.
pub(crate) fn valid_content_hash(s: &str) -> bool {
    s.len() == 71 && s.starts_with("sha256:") && s[7..].bytes().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

pub(crate) fn number_calc_ok(calc: &str) -> bool {
    let has_digit = calc.bytes().any(|b| b.is_ascii_digit());
    let has_operator = calc.contains(['+', '-', '*', '/', '\u{f7}', '\u{d7}', '=', '\u{2248}', '%']);
    !calc.trim().is_empty() && has_digit && has_operator
}

pub(crate) fn structural_enforced_by_ok(s: &str) -> bool {
    if s.trim().is_empty() {
        return false;
    }
    if s.starts_with("type:") || s.starts_with("gate:") {
        return true;
    }
    for ext in [".ts", ".tsx", ".py", ".rs", ".json"] {
        if let Some(idx) = s.find(ext) {
            let rest = &s[idx + ext.len()..];
            if rest.is_empty() {
                return true;
            }
            if let Some(digits) = rest.strip_prefix(':') {
                if !digits.is_empty() && digits.bytes().all(|c| c.is_ascii_digit()) {
                    return true;
                }
            }
        }
    }
    false
}
