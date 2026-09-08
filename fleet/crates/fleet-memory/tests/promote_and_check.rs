//! `promote_lesson()`/`check_added_line()` cases, plus a promote-then-check end-to-end flow.
//! See BLUEPRINT.md §9.

mod support;

use fleet_memory::{check_added_line, promote_lesson, DiffPattern, PatternError, PatternMatcher, PromotionRefusal, PromotionScope};
use support::item;

struct SubstringMatcher;
impl PatternMatcher for SubstringMatcher {
    fn is_match(&self, pattern: &DiffPattern, line: &str) -> Result<bool, PatternError> {
        Ok(line.contains(pattern.as_str()))
    }
}

struct AlwaysHit;
impl PatternMatcher for AlwaysHit {
    fn is_match(&self, _p: &DiffPattern, _line: &str) -> Result<bool, PatternError> {
        Ok(true)
    }
}

#[test]
fn promote_lesson_refuses_before_checking_confirmations_when_pattern_is_empty() {
    let mut it = item("m1");
    it.confirmed_count = 0;
    let err = promote_lesson(&it, "", "cat", PromotionScope::Source, 5).unwrap_err();
    assert_eq!(err, PromotionRefusal::EmptyPattern);
}

#[test]
fn promote_lesson_accepts_zero_min_confirmations() {
    let mut it = item("m1");
    it.confirmed_count = 0;
    let lesson = promote_lesson(&it, "foo.*bar", "cat", PromotionScope::Source, 0).unwrap();
    assert_eq!(lesson.category, "cat");
}

#[test]
fn check_added_line_wraps_a_hit_as_gate_refusal_named_by_category() {
    let it = item("m1");
    let lesson = promote_lesson(&it, "foo", "my_category", PromotionScope::Source, 0).unwrap();
    let refusal = check_added_line(&lesson, "any line", &AlwaysHit).unwrap().unwrap();
    assert_eq!(refusal.code(), "my_category");
}

#[test]
fn promote_then_check_end_to_end() {
    let mut it = item("m1");
    it.confirmed_count = 3;
    let lesson = promote_lesson(&it, "forbidden_call", "gate_x", PromotionScope::Source, 2).unwrap();
    let matcher = SubstringMatcher;
    let hit = check_added_line(&lesson, "let x = forbidden_call();", &matcher).unwrap();
    assert!(hit.is_some());
    let clean = check_added_line(&lesson, "let x = safe_call();", &matcher).unwrap();
    assert!(clean.is_none());
}
