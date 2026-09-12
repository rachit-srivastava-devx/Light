use ready::{evaluate, ReadyInput, Status, Violation};

fn passing() -> ReadyInput {
    ReadyInput {
        acceptance: vec!["ac".to_string()],
        questions_open: 0,
        reviewer_accepts: true,
        reviewer_digest: "d".to_string(),
        plan_digest: "d".to_string(),
        deps_pinned: true,
        grants_cover: true,
        write_scope_exclusive: true,
        resources_available: true,
        resource_profile: "default".to_string(),
        checked: 7,
        total: 7,
    }
}

#[test]
fn empty_acceptance_list_causes_not_ready() {
    let v = evaluate(&ReadyInput { acceptance: vec![], ..passing() });
    assert_eq!(v.status, Status::NotReady);
    assert!(v.violations.contains(&Violation::EmptyAcceptance));
}

#[test]
fn open_questions_cause_not_ready() {
    let v = evaluate(&ReadyInput { questions_open: 1, ..passing() });
    assert_eq!(v.status, Status::NotReady);
    assert!(v.violations.iter().any(|viol| matches!(viol, Violation::OpenQuestions(_))));
}

#[test]
fn deps_not_pinned_causes_not_ready() {
    let v = evaluate(&ReadyInput { deps_pinned: false, ..passing() });
    assert_eq!(v.status, Status::NotReady);
    assert!(v.violations.contains(&Violation::DepsNotPinned));
}

#[test]
fn grants_not_covered_causes_not_ready() {
    let v = evaluate(&ReadyInput { grants_cover: false, ..passing() });
    assert_eq!(v.status, Status::NotReady);
    assert!(v.violations.contains(&Violation::GrantsNotCovered));
}

#[test]
fn write_conflict_causes_not_ready() {
    let v = evaluate(&ReadyInput { write_scope_exclusive: false, ..passing() });
    assert_eq!(v.status, Status::NotReady);
    assert!(v.violations.contains(&Violation::WriteNotExclusive));
}

#[test]
fn resources_unavailable_causes_not_ready() {
    let v = evaluate(&ReadyInput { resources_available: false, ..passing() });
    assert_eq!(v.status, Status::NotReady);
    assert!(v.violations.contains(&Violation::ResourcesUnavailable));
}
