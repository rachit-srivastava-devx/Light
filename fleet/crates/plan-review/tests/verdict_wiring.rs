//! Kills Decision-is-never-read mutations: emit_walkthrough must derive
//! `approved` from Decision::Accept, not from a caller-supplied literal.
use plan_review::{
    Decision, PlanProposal, ReviewVerdict, ReviewEvent, ReviewedPlanDigest,
    emit_walkthrough,
};

fn proposal(plan_digest: &str) -> PlanProposal {
    PlanProposal {
        plan_digest: plan_digest.into(),
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

fn get_digest(evs: &[ReviewEvent]) -> &ReviewedPlanDigest {
    evs.iter().find_map(|e| {
        if let ReviewEvent::Digest(d) = e { Some(d) } else { None }
    }).unwrap()
}

// Kills: emit_walkthrough body → approved = true (constant)
#[test]
fn approved_decision_yields_approved_digest() {
    let evs = emit_walkthrough(&proposal("p1"), "r1", &verdict(Decision::Accept));
    let d = get_digest(&evs);
    assert!(d.approved, "Decision::Accept must set approved=true");
}

// Kills: emit_walkthrough body → approved = false (constant) by the complementary case
// Also kills: approved = true (by catching both as exact-value pins)
#[test]
fn rejected_decision_yields_not_approved_digest() {
    let evs = emit_walkthrough(&proposal("p1"), "r1", &verdict(Decision::Reject));
    let d = get_digest(&evs);
    assert!(!d.approved, "Decision::Reject must set approved=false");
}

// Kills: plan_digest field copied from wrong source
#[test]
fn digest_plan_digest_matches_input() {
    let evs = emit_walkthrough(&proposal("my-plan-hash"), "r1", &verdict(Decision::Accept));
    let d = get_digest(&evs);
    assert_eq!(d.plan_digest, "my-plan-hash");
}

// Kills: checked/total fields stubbed to 0
#[test]
fn checked_total_propagated_from_verdict() {
    let mut v = verdict(Decision::Accept);
    v.checked = 5;
    v.total = 7;
    let evs = emit_walkthrough(&proposal("p1"), "r1", &v);
    let d = get_digest(&evs);
    assert_eq!(d.checked, 5);
    assert_eq!(d.total, 7);
}
