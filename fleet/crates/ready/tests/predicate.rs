use ready::{evaluate, ReadyInput, Status, Violation};

/// Build a fully-passing input with the given overrides applied by the caller.
fn passing_input() -> ReadyInput {
    ReadyInput {
        acceptance: vec!["acceptance-criterion-1".to_string()],
        questions_open: 0,
        reviewer_accepts: true,
        reviewer_digest: "abc123".to_string(),
        plan_digest: "abc123".to_string(),
        deps_pinned: true,
        grants_cover: true,
        write_scope_exclusive: true,
        resources_available: true,
        checked: 7,
        total: 7,
    }
}

/// A single false predicate must produce `NotReady` with exactly one named
/// `Violation`. Catches any "always return Ready" stub.
#[test]
fn one_false_predicate_refuses() {
    let input = ReadyInput {
        reviewer_accepts: false,
        ..passing_input()
    };
    let verdict = evaluate(&input);
    assert_eq!(
        verdict.status,
        Status::NotReady,
        "one false predicate must produce NotReady"
    );
    assert_eq!(
        verdict.violations,
        vec![Violation::ReviewerRejected],
        "exactly the ReviewerRejected violation must be raised"
    );
}

/// A zero denominator (`checked=0, total=0`) must be refused even when every
/// boolean predicate is true. Catches accepting `0/0` as a green gate.
#[test]
fn zero_denominator_is_rejected() {
    let input = ReadyInput {
        checked: 0,
        total: 0,
        ..passing_input()
    };
    let verdict = evaluate(&input);
    assert_eq!(
        verdict.status,
        Status::ZeroCoverage,
        "zero denominator must produce ZeroCoverage, not Ready"
    );
}

/// `reviewer_accepts: true` with a mismatched digest must produce `NotReady`
/// with a `StaleDigest` violation. Catches skipping the digest comparison.
#[test]
fn stale_reviewer_digest_refuses() {
    let input = ReadyInput {
        reviewer_accepts: true,
        reviewer_digest: "old-digest-v1".to_string(),
        plan_digest: "new-digest-v2".to_string(),
        ..passing_input()
    };
    let verdict = evaluate(&input);
    assert_eq!(
        verdict.status,
        Status::NotReady,
        "stale reviewer digest must produce NotReady"
    );
    assert!(
        verdict.violations.contains(&Violation::StaleDigest),
        "StaleDigest violation must be present; got {:?}",
        verdict.violations
    );
}
