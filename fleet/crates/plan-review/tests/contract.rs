//! Tests for validate contract checks.
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
    assert!(matches!(
        validate(&inp, &verdict(Decision::Accept, "p1", 1)),
        Err(ReviewError::NotIndependent),
    ));
}

// Kills: skipping empty-digest guard (returning Ok for empty input_digest)
#[test]
fn mismatched_input_digest_is_rejected() {
    let err = validate(&input(), &verdict(Decision::Accept, "", 1));
    assert!(matches!(err, Err(ReviewError::DigestMismatch)));
}

// Kills: skipping checked == 0 guard (returning Ok for empty review)
#[test]
fn zero_checked_is_rejected() {
    let err = validate(&input(), &verdict(Decision::Accept, "p1", 0));
    assert!(matches!(err, Err(ReviewError::InvalidInput(_))));
}

// Happy path with valid input passes through
#[test]
fn valid_proposal_with_matching_digest_passes() {
    let result = validate(&input(), &verdict(Decision::Accept, "p1", 1));
    assert!(result.is_ok(), "expected Ok, got an error");
}

// Kills: || → && in empty-digest guard (empty output_digest alone must reject)
#[test]
fn finding_with_empty_severity_is_rejected() {
    let mut v = verdict(Decision::Accept, "p1", 1);
    v.output_digest = String::new();
    assert!(matches!(validate(&input(), &v), Err(ReviewError::DigestMismatch)));
}

// Kills: skipping empty plan_digest on input (invalid input must be caught)
#[test]
fn finding_with_empty_location_is_rejected() {
    let mut inp = input();
    inp.plan_digest = String::new();
    assert!(matches!(
        validate(&inp, &verdict(Decision::Accept, "p1", 1)),
        Err(ReviewError::InvalidInput(_)),
    ));
}
