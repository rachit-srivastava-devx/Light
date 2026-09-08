use fleet_plan::{submission_eligible, validate_review_contract, verdict_decision, RoleContract, VerdictDecision, VerdictName, VerdictRefusal};
use fleet_types::{LifecycleState, TaskId};
use std::collections::BTreeSet;

fn set(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|s| s.to_string()).collect()
}

#[test]
fn full_submit_to_accept_happy_path() {
    let worker = RoleContract {
        reviewer: "verifier".into(),
        may_produce: set(&["work_output"]),
        must_consume: BTreeSet::new(),
        cycle_states: BTreeSet::new(),
    };
    let reviewer = RoleContract {
        reviewer: String::new(),
        may_produce: BTreeSet::new(),
        must_consume: set(&["work_output"]),
        cycle_states: set(&["review"]),
    };
    assert!(validate_review_contract("builder", &worker, "verifier", &reviewer).is_ok());

    let worker_id = TaskId::parse("worker-1").unwrap();
    let reviewer_id = TaskId::parse("reviewer-1").unwrap();
    assert!(submission_eligible(&worker_id, &reviewer_id, LifecycleState::Verifying, LifecycleState::Intake, 0, 3).is_ok());

    let decision = verdict_decision("looks good", LifecycleState::Verified, VerdictName::Accept, 0, 3, "plan").unwrap();
    assert_eq!(decision, VerdictDecision::Accept);
}

#[test]
fn submission_eligible_boundary_at_exact_limit() {
    let worker_id = TaskId::parse("w").unwrap();
    let reviewer_id = TaskId::parse("r").unwrap();
    let refused = submission_eligible(&worker_id, &reviewer_id, LifecycleState::Verifying, LifecycleState::Intake, 3, 3);
    assert!(refused.is_err());
    let ok = submission_eligible(&worker_id, &reviewer_id, LifecycleState::Verifying, LifecycleState::Intake, 2, 3);
    assert!(ok.is_ok());
}

#[test]
fn submission_eligible_rejects_self_review() {
    let id = TaskId::parse("same").unwrap();
    let r = submission_eligible(&id, &id, LifecycleState::Verifying, LifecycleState::Intake, 0, 3);
    assert!(r.is_err());
}

#[test]
fn verdict_decision_revise_past_limit_escalates_not_revises() {
    let d = verdict_decision("r", LifecycleState::Verified, VerdictName::Revise, 3, 3, "plan").unwrap();
    assert_eq!(d, VerdictDecision::Escalate { attempts: 3, limit: 3 });
}

#[test]
fn verdict_decision_reason_missing_wins_over_every_other_check() {
    let err = verdict_decision("", LifecycleState::Intake, VerdictName::Accept, 99, 1, "nonsense").unwrap_err();
    assert_eq!(err, VerdictRefusal::ReasonMissing);
}

#[test]
fn verdict_decision_reenter_state_must_be_plan() {
    let err = verdict_decision("r", LifecycleState::Verified, VerdictName::Revise, 0, 3, "somewhere-else").unwrap_err();
    assert!(matches!(err, VerdictRefusal::ReenterStateUnsupported(_)));
}
