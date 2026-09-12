use crate::types::{ReadyInput, ReadyVerdict, Status, Violation};
use crate::receipt::validate_denominator;

/// Evaluate all fixed predicates. Returns ZeroCoverage when checked==0 or total==0.
/// Pure, deterministic, no IO, no panic.
pub fn evaluate(input: &ReadyInput) -> ReadyVerdict {
    if validate_denominator(input).is_err() {
        return ReadyVerdict {
            status: Status::ZeroCoverage,
            violations: vec![],
            checked: input.checked,
            total: input.total,
        };
    }
    let mut violations = Vec::new();
    if input.acceptance.is_empty() {
        violations.push(Violation::EmptyAcceptance);
    }
    if input.questions_open > 0 {
        violations.push(Violation::OpenQuestions(input.questions_open));
    }
    if !input.reviewer_accepts {
        violations.push(Violation::ReviewerNotAccepted);
    } else if input.reviewer_digest != input.plan_digest {
        violations.push(Violation::StaleDigest);
    }
    if !input.deps_pinned {
        violations.push(Violation::DepsNotPinned);
    }
    if !input.grants_cover {
        violations.push(Violation::GrantsNotCovered);
    }
    if !input.write_scope_exclusive {
        violations.push(Violation::WriteNotExclusive);
    }
    if !input.resources_available {
        violations.push(Violation::ResourcesUnavailable);
    }
    let status = if violations.is_empty() { Status::Ready } else { Status::NotReady };
    ReadyVerdict { status, violations, checked: input.checked, total: input.total }
}
