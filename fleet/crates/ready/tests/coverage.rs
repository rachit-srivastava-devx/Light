use ready::{evaluate, ReadyInput, Receipt, Status};

fn passing() -> ReadyInput {
    ReadyInput {
        acceptance: vec!["ac-1".to_string()],
        questions_open: 0,
        reviewer_accepts: true,
        reviewer_digest: "d1".to_string(),
        plan_digest: "d1".to_string(),
        deps_pinned: true,
        grants_cover: true,
        write_scope_exclusive: true,
        resources_available: true,
        resource_profile: "default".to_string(),
        checked: 7,
        total: 7,
    }
}

/// Pins predicate count at 7: asserts checked==7 AND total==7 in a passing verdict.
/// Also kills the `checked == total` → `checked != total` mutation in validate_denominator.
#[test]
fn all_seven_predicates_pass_with_correct_counts() {
    let v = evaluate(&passing());
    assert_eq!(v.status, Status::Ready, "expected Ready");
    assert_eq!(v.checked, 7, "checked must be exactly 7");
    assert_eq!(v.total, 7, "total must be exactly 7");
    assert!(v.violations.is_empty(), "no violations expected");
}

/// checked=0, total=7 → ZeroCoverage (zero-checked is not a passing gate).
#[test]
fn checked_zero_with_nonzero_total_is_zero_coverage() {
    let v = evaluate(&ReadyInput { checked: 0, total: 7, ..passing() });
    assert_eq!(v.status, Status::ZeroCoverage);
}

/// checked=5, total=7 → ZeroCoverage (partial coverage is refused).
#[test]
fn checked_partial_mismatch_is_zero_coverage() {
    let v = evaluate(&ReadyInput { checked: 5, total: 7, ..passing() });
    assert_eq!(v.status, Status::ZeroCoverage);
}

/// Status::ZeroCoverage must be constructible — prevents dead-code elimination of the variant.
#[test]
fn incomplete_status_variant_is_constructible() {
    let s = Status::ZeroCoverage;
    assert_eq!(format!("{s:?}"), "ZeroCoverage");
}

/// Receipt serialised must produce non-empty output containing the status.
/// Kills the stubbed-status and empty-serialisation mutations.
#[test]
fn receipt_to_json_contains_status() {
    let input = passing();
    let verdict = evaluate(&input);
    let receipt = Receipt::from_verdict(&input, &verdict);
    let json = serde_json::to_string(&receipt).unwrap();
    assert!(!json.is_empty(), "serialised receipt must not be empty");
    assert!(json.contains("Ready"), "serialised receipt must contain the status; got: {json}");
}
