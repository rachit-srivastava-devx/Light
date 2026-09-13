//! The numeric-derivation/structural-enforcement predicates, split from `module_brief_core.rs`'s
//! small string/JSON accessors. Verbatim from `lld.rs:37-127`.

pub(crate) fn number_calc_ok(calc: &str) -> bool {
    let has_digit = calc.bytes().any(|b| b.is_ascii_digit());
    let has_operator =
        calc.contains(['+', '-', '*', '/', '\u{f7}', '\u{d7}', '=', '\u{2248}', '%']);
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
