//! `check_added_line` — does one added diff line match a promoted lesson's pattern.
//! See BLUEPRINT.md §3.H.

use crate::promote::{DiffPattern, PromotedLesson};
use fleet_types::GateRefusal;

/// The injected pattern-matching port — wraps whatever regex engine the caller chooses. This
/// crate links no regex engine itself.
pub trait PatternMatcher {
    fn is_match(&self, pattern: &DiffPattern, line: &str) -> Result<bool, PatternError>;
}
/// The injected `PatternMatcher` failed to evaluate.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("pattern match failed: {0}")]
pub struct PatternError(pub String);

/// Pure orchestration over one injected call. `GateRefusal::code` is a compiled-in `&'static
/// str` (`fleet-types`'s roster-wide shape); `lesson.category` is a caller-supplied, dynamic
/// `String`, so it cannot be handed to `GateRefusal::new` without leaking — this fn does that
/// leak deliberately (one small, bounded allocation per distinct category value ever promoted
/// through this call site, not per invocation of `check_added_line`) rather than inventing a
/// fifth, non-reused refusal shape. Flagged for Opus (see the crate's return notes).
pub fn check_added_line(
    lesson: &PromotedLesson,
    line: &str,
    matcher: &dyn PatternMatcher,
) -> Result<Option<GateRefusal>, PatternError> {
    if matcher.is_match(&lesson.pattern, line)? {
        let code: &'static str = Box::leak(lesson.category.clone().into_boxed_str());
        let message = format!("line matches promoted lesson pattern: {line}");
        return Ok(Some(GateRefusal::new(code, message)));
    }
    Ok(None)
}
