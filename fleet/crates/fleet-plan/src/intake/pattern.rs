//! Hand-written matcher engine standing in for the 13 fixed EREs `write_questions` uses
//! (`intake.sh:111-123`). This workspace has no `regex` dependency (the same convention
//! `lld.rs:59-63` documents), so each ERE alternative is expressed as one of a small closed set
//! of matcher shapes and evaluated by hand. Returns the leftmost match across all alternatives,
//! matching `first_match`'s left-to-right, first-occurrence semantics (`intake.sh:68`).

use super::pattern_custom::leftmost;

pub(crate) enum Matcher {
    /// `\bword\b` -- an exact whole word.
    Word(&'static str),
    /// `\bstem s?\b` -- `stem` or `stem` + `"s"`, as a whole word.
    WordOptS(&'static str),
    /// `\bstem[a-z]*\b` -- a whole word beginning with `stem`.
    Stem(&'static str),
    /// A plain substring, unanchored (no `\b` in the source ERE).
    Literal(&'static str),
    /// `--[a-z][a-z0-9_-]*` -- a long CLI flag.
    CliFlag,
    /// `clean ?up` -- "cleanup" or "clean up".
    CleanUp,
}

pub(crate) fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

pub(crate) fn word_runs(text: &str) -> Vec<(usize, usize)> {
    let mut runs = Vec::new();
    let mut start: Option<usize> = None;
    for (i, c) in text.char_indices() {
        if is_word_char(c) {
            if start.is_none() {
                start = Some(i);
            }
        } else if let Some(s) = start.take() {
            runs.push((s, i));
        }
    }
    if let Some(s) = start {
        runs.push((s, text.len()));
    }
    runs
}

/// Leftmost match across `matchers`, ties broken by earlier position in `matchers` -- mirrors
/// grep -oE's leftmost-first-alternative semantics closely enough for this fixed, hand-audited
/// vocabulary (none of these 13 patterns overlap at the same starting position in practice).
pub(crate) fn first_match(matchers: &[Matcher], text_lc: &str) -> Option<String> {
    matchers
        .iter()
        .filter_map(|m| leftmost(text_lc, m))
        .min_by_key(|&(s, _)| s)
        .map(|(s, e)| text_lc[s..e].to_string())
}
