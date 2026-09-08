//! `promote_lesson` — lesson -> enforceable diff-pattern gate. See BLUEPRINT.md §3.H.

use crate::ident::EmptyMemoryId;
use crate::item::MemoryItem;

/// A caller-supplied, not-yet-compiled diff-matching pattern (an ERE string). This crate
/// validates only non-emptiness — compiling/running the pattern is the injected
/// `PatternMatcher`'s job.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiffPattern(String);
impl DiffPattern {
    pub fn parse(value: impl Into<String>) -> Result<Self, EmptyMemoryId> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(EmptyMemoryId);
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Mirrors `memory-check.sh:175`'s `[ "$scope" = source ]` gate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PromotionScope {
    Source,
    Other,
}

/// A promoted, enforceable lesson — the pure data this crate hands to a gate (`fleet-verify`).
#[derive(Clone, Debug)]
pub struct PromotedLesson {
    pub category: String,
    pub pattern: DiffPattern,
    pub scope: PromotionScope,
    pub message: String,
}

/// Why a memory did not qualify for promotion.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum PromotionRefusal {
    #[error("memory confirmed {actual} time(s), promotion requires at least {required}")]
    NotConfirmedEnough { required: u32, actual: u32 },
    #[error("promotion pattern must not be empty")]
    EmptyPattern,
    #[error("promotion category must not be empty")]
    EmptyCategory,
}

/// Pure and total. Checks pattern/category emptiness before the confirmation-count bar (cheapest
/// problem surfaced first), then refuses if `item.confirmed_count < min_confirmations`.
pub fn promote_lesson(
    item: &MemoryItem,
    pattern: &str,
    category: &str,
    scope: PromotionScope,
    min_confirmations: u32,
) -> Result<PromotedLesson, PromotionRefusal> {
    if pattern.trim().is_empty() {
        return Err(PromotionRefusal::EmptyPattern);
    }
    if category.trim().is_empty() {
        return Err(PromotionRefusal::EmptyCategory);
    }
    if item.confirmed_count < min_confirmations {
        return Err(PromotionRefusal::NotConfirmedEnough { required: min_confirmations, actual: item.confirmed_count });
    }
    Ok(PromotedLesson {
        category: category.to_string(),
        pattern: DiffPattern::parse(pattern).map_err(|_| PromotionRefusal::EmptyPattern)?,
        scope,
        message: format!("lesson from memory {} violated by an added line", item.id.as_str()),
    })
}
