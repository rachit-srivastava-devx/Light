//! Kills Decision-is-never-read mutations: verdict_to_digest must derive
//! `approved` from Decision::Approved, not from a caller-supplied literal.
use plan_review::{
    Decision, ReviewInput, ReviewVerdict, verdict_to_digest,
};

fn input(plan_digest: &str) -> ReviewInput {
    ReviewInput {
        plan_digest: plan_digest.into(),
        reviewer_id: "r1".into(),
        worker_id: "w1".into(),
        evidence_digest: "e1".into(),
    }
}

fn verdict(decision: Decision) -> ReviewVerdict {
    ReviewVerdict {
        decision,
        findings: vec![],
        input_digest: "p1".into(),
        output_digest: "o1".into(),
        checked: 1,
        total: 1,
    }
}

// Kills: verdict_to_digest body → approved = true (constant)
#[test]
fn approved_decision_yields_approved_digest() {
    let d = verdict_to_digest(&input("p1"), &verdict(Decision::Approved), "r1");
    assert!(d.approved, "Decision::Approved must set approved=true");
}

// Kills: verdict_to_digest body → approved = false (constant) by the complementary case
// Also kills: approved = true (by catching both as exact-value pins)
#[test]
fn rejected_decision_yields_not_approved_digest() {
    let d = verdict_to_digest(&input("p1"), &verdict(Decision::Rejected), "r1");
    assert!(!d.approved, "Decision::Rejected must set approved=false");
}

// Kills: plan_digest field copied from wrong source
#[test]
fn digest_plan_digest_matches_input() {
    let d = verdict_to_digest(&input("my-plan-hash"), &verdict(Decision::Approved), "r1");
    assert_eq!(d.plan_digest, "my-plan-hash");
}

// Kills: checked/total fields stubbed to 0
#[test]
fn checked_total_propagated_from_verdict() {
    let mut v = verdict(Decision::Approved);
    v.checked = 5;
    v.total = 7;
    let d = verdict_to_digest(&input("p1"), &v, "r1");
    assert_eq!(d.checked, 5);
    assert_eq!(d.total, 7);
}
