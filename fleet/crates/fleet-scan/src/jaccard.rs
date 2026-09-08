//! Word-tokenization and Jaccard similarity used by the merge dedup step.

use std::collections::BTreeSet;

/// Lowercase, strip non-alphanumeric runs into word boundaries. Unicode-aware via
/// `char::is_alphanumeric` -- not ASCII-only.
pub(crate) fn tokenize(s: &str) -> BTreeSet<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// Word-tokenize (lowercase, strip non-alphanumeric) and compute Jaccard similarity
/// `|A∩B| / |A∪B|` between two strings' token sets. `0.0` if both are empty (defined, not NaN).
/// Pure, total, no allocation beyond the two token sets.
pub fn jaccard_similarity(a: &str, b: &str) -> f32 {
    let ta = tokenize(a);
    let tb = tokenize(b);
    if ta.is_empty() && tb.is_empty() {
        return 0.0;
    }
    let intersection = ta.intersection(&tb).count();
    let union = ta.union(&tb).count();
    if union == 0 {
        return 0.0;
    }
    intersection as f32 / union as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symmetric_and_bounded() {
        let cases = [("a b c", "b c d"), ("hello world", "hello world"), ("", "")];
        for (a, b) in cases {
            let ab = jaccard_similarity(a, b);
            let ba = jaccard_similarity(b, a);
            assert_eq!(ab, ba);
            assert!((0.0..=1.0).contains(&ab));
        }
        assert_eq!(jaccard_similarity("", ""), 0.0);
    }

    #[test]
    fn unicode_tokens_compare() {
        let sim = jaccard_similarity("café naïve", "café naïve résumé");
        assert!(sim > 0.0 && sim < 1.0);
    }
}
