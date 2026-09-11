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

/// `l.get(..n)`/`l.get(n..)`, never `l[..n]`: `prefix.len()` is a BYTE count, and a SOW line
/// whose first `n` bytes end inside a multi-byte char (an em-dash heading, an emoji, an accented
/// word) made the slicing form panic the whole process. `get` yields `None` there instead, which
/// is the same verdict as "this line does not start with the prefix". Semantics are otherwise
/// unchanged: the ASCII-case-insensitive head must match and the remainder must be non-blank
/// (a line exactly equal to the prefix leaves an empty remainder, so it still fails, as before).
pub(crate) fn has_line_prefix_ci(text: &str, prefix: &str) -> bool {
    text.lines().any(|l| {
        let n = prefix.len();
        matches!(l.get(..n), Some(head) if head.eq_ignore_ascii_case(prefix))
            && matches!(l.get(n..), Some(rest) if !rest.trim().is_empty())
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
