use crate::Effect;

// Keyword table: (kind, keywords-that-imply-it) in conservatism order.
const KIND_KEYWORDS: &[(&str, &[&str])] = &[
    ("answer",           &["what is", "how does", "explain", "describe"]),
    ("review-only",      &["review", "audit", "check only"]),
    ("small-change",     &["fix", "bug", "patch", "typo", "small"]),
    ("investigate",      &["investigate", "diagnose", "trace"]),
    ("research-design",  &["research", "explore", "design proposal"]),
    ("refactor",         &["refactor", "reorganize", "restructure"]),
    ("feature",          &["new feature", "implement feature", "add feature"]),
    ("incident",         &["incident", "outage", "emergency", "rollback"]),
    ("multi-repo-change",&["cross-repo", "multi-repo", "monorepo-wide"]),
];

/// Deterministic kind classification based on request text only.
/// Ignores model confidence. Disagreement with model kind → caller picks conservative.
pub fn independent_kind(request: &str, _effects: &[Effect]) -> String {
    let lower = request.to_lowercase();
    for (kind, keywords) in KIND_KEYWORDS {
        if keywords.iter().any(|kw| lower.contains(kw)) {
            return kind.to_string();
        }
    }
    "investigate".to_string() // conservative default
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fix_keyword_maps_to_small_change() {
        assert_eq!(independent_kind("fix a small bug", &[]), "small-change");
    }
    #[test]
    fn unknown_text_defaults_to_investigate() {
        assert_eq!(independent_kind("do something unspecified", &[]), "investigate");
    }
}
