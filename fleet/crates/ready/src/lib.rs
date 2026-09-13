//! `ready` — deterministic readiness gate for fleet modules.
//!
//! STATUS: no workspace member depends on this crate yet, matching `docs/LLD/LLD.md` §7/§8's
//! not-yet-wired scheduling design — not dead code.
//!
//! Evaluates seven fixed boolean predicates and a checked/total denominator.
//! No model call, no IO, no panic. A zero-input gate always fails.
//! Blueprint: docs/blueprints-next/ready/BLUEPRINT.md

mod predicate;
mod receipt;
mod types;

pub use predicate::evaluate;
pub use receipt::{validate_denominator, Receipt};
pub use types::{ReadyInput, ReadyVerdict, Status, Violation};

#[cfg(test)]
mod tests {
    use super::*;

    fn all_pass(checked: u64) -> ReadyInput {
        ReadyInput {
            acceptance: vec!["ac1".into()],
            questions_open: 0,
            reviewer_accepts: true,
            deps_pinned: true,
            grants_cover: true,
            write_scope_exclusive: true,
            resources_available: true,
            resource_profile: "default".into(),
            checked,
            total: checked,
            plan_digest: "d1".into(),
            reviewer_digest: "d1".into(),
        }
    }

    #[test]
    fn one_false_predicate_refuses() {
        let mut input = all_pass(7);
        input.reviewer_accepts = false;
        let v = evaluate(&input);
        assert_eq!(v.status, Status::NotReady);
        assert!(v.violations.contains(&Violation::ReviewerNotAccepted));
    }

    #[test]
    fn zero_denominator_is_rejected() {
        let mut input = all_pass(0);
        input.checked = 0;
        input.total = 0;
        let v = evaluate(&input);
        assert_eq!(v.status, Status::ZeroCoverage);
    }

    #[test]
    fn stale_reviewer_digest_refuses() {
        let mut input = all_pass(7);
        input.reviewer_digest = "different".into();
        let v = evaluate(&input);
        assert_eq!(v.status, Status::NotReady);
        assert!(v.violations.contains(&Violation::StaleDigest));
    }
}
