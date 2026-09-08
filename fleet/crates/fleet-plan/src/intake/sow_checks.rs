//! Small text predicates `validate_sow_text` composes -- split out of `sow.rs` to stay under the
//! 80-line file cap.

pub(crate) fn section_body(text: &str, heading: &str) -> Option<String> {
    let want = format!("## {heading}");
    let mut lines = text.lines();
    for line in lines.by_ref() {
        if line.trim_end().eq_ignore_ascii_case(&want) {
            let mut body = String::new();
            for l in lines.by_ref() {
                if l.starts_with("## ") {
                    break;
                }
                body.push_str(l);
            }
            return Some(body);
        }
    }
    None
}

pub(crate) fn has_line_prefix_ci(text: &str, prefix: &str) -> bool {
    text.lines().any(|l| {
        l.len() > prefix.len()
            && l[..prefix.len()].eq_ignore_ascii_case(prefix)
            && !l[prefix.len()..].trim().is_empty()
    })
}

pub(crate) fn contains_any_ci(text: &str, needles: &[&str]) -> bool {
    let lc = text.to_lowercase();
    needles.iter().any(|n| lc.contains(n))
}

/// `[0-9]+%|p95|exit[[:space:]]+0|100%` -- a digit run immediately followed by `%`, or one of the
/// two fixed literals.
pub(crate) fn has_measurable_threshold(text: &str) -> bool {
    let lc = text.to_lowercase();
    if contains_any_ci(&lc, &["p95", "exit 0"]) {
        return true;
    }
    let b = lc.as_bytes();
    b.iter().enumerate().any(|(i, &c)| c == b'%' && i > 0 && b[i - 1].is_ascii_digit())
}
