//! Tests for validate_plan_proposal contract checks.
//! Kills: skipping reviewer_id check, skipping digest comparison, returning Ok on empty checked.
use plan_review::{Decision, ReviewError, ReviewInput, ReviewVerdict, validate};

fn input() -> ReviewInput {
    ReviewInput {
        plan_digest: "p1".into(),
        reviewer_id: "r1".into(),
        worker_id: "w1".into(),
        evidence_digest: "e1".into(),
    }
}

fn verdict(decision: Decision, input_digest: &str, checked: u64) -> ReviewVerdict {
    ReviewVerdict {
        decision,
        findings: vec![],
        input_digest: input_digest.into(),
        output_digest: "o1".into(),
        checked,
        total: 1,
    }
}

// Kills: skipping reviewer_id == worker_id check
#[test]
fn self_review_is_rejected() {
    let mut inp = input();
    inp.reviewer_id = inp.worker_id.clone();
    assert_eq!(
        validate(&inp, &verdict(Decision::Approved, "p1", 1)),
        Err(ReviewError::NotIndependent),
    );
}

// Kills: skipping verdict.input_digest != input.plan_digest check
#[test]
fn mismatched_input_digest_is_rejected() {
    let err = validate(&input(), &verdict(Decision::Approved, "WRONG-DIGEST", 1));
    assert_eq!(err, Err(ReviewError::DigestMismatch));
}

// Kills: skipping checked == 0 guard (returning Ok for empty review)
#[test]
fn zero_checked_is_rejected() {
    let err = validate(&input(), &verdict(Decision::Approved, "p1", 0));
    assert_eq!(err, Err(ReviewError::MalformedFinding));
}

// Happy path with matching digest passes through
#[test]
fn valid_proposal_with_matching_digest_passes() {
    let result = validate(&input(), &verdict(Decision::Approved, "p1", 1));
    assert_eq!(result, Ok(()));
}

fn verdict_with_finding(severity: &str, location: &str) -> ReviewVerdict {
    ReviewVerdict {
        decision: Decision::Approved,
        findings: vec![plan_review::Finding { severity: severity.into(), location: location.into(), message: "m".into() }],
        input_digest: "p1".into(), output_digest: "o1".into(), checked: 1, total: 1,
    }
}
// Kills: || → && (either field empty alone must reject)
#[test]
fn finding_with_empty_severity_is_rejected() {
    assert_eq!(validate(&input(), &verdict_with_finding("", "src/lib.rs")), Err(ReviewError::MalformedFinding));
}
#[test]
fn finding_with_empty_location_is_rejected() {
    assert_eq!(validate(&input(), &verdict_with_finding("critical", "")), Err(ReviewError::MalformedFinding));
}
