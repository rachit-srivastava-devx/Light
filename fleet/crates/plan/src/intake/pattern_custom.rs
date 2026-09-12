//! The non-word-run matcher shapes (`CliFlag`, `CleanUp`) plus `leftmost`'s dispatch over every
//! `Matcher` variant -- split out of `pattern.rs` to stay under the 80-line file cap.

use super::pattern::{word_runs, Matcher};

pub(crate) fn leftmost(text: &str, m: &Matcher) -> Option<(usize, usize)> {
    match m {
        Matcher::Word(w) => word_runs(text).into_iter().find(|&(s, e)| &text[s..e] == *w),
        Matcher::WordOptS(stem) => word_runs(text).into_iter().find(|&(s, e)| {
            let run = &text[s..e];
            run == *stem || run == format!("{stem}s")
        }),
        Matcher::Stem(stem) => word_runs(text)
            .into_iter()
            .find(|&(s, e)| text[s..e].starts_with(stem)),
        Matcher::Literal(lit) => text.find(lit).map(|p| (p, p + lit.len())),
        Matcher::CliFlag => find_cli_flag(text),
        Matcher::CleanUp => {
            let a = text.find("cleanup").map(|p| (p, p + "cleanup".len()));
            let b = text.find("clean up").map(|p| (p, p + "clean up".len()));
            match (a, b) {
                (Some(a), Some(b)) => Some(if a.0 <= b.0 { a } else { b }),
                (Some(a), None) => Some(a),
                (None, Some(b)) => Some(b),
                (None, None) => None,
            }
        }
    }
}

fn find_cli_flag(text: &str) -> Option<(usize, usize)> {
    let b = text.as_bytes();
    let mut i = 0;
    while i + 2 < b.len() {
        if &b[i..i + 2] == b"--" && b[i + 2].is_ascii_lowercase() {
            let mut end = i + 3;
            while end < b.len() && is_flag_char(b[end]) {
                end += 1;
            }
            return Some((i, end));
        }
        i += 1;
    }
    None
}

fn is_flag_char(c: u8) -> bool {
    c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' || c == b'-'
}
