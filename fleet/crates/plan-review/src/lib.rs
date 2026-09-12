//! `plan-review` — independent review gate for plan proposals.
//! Blueprint: docs/blueprints-next/plan_review/BLUEPRINT.md
//! C9 enforced: all model calls route through the injected PlanReviewer port.

pub mod contract;
pub mod types;
pub mod verdict;

pub use contract::{validate, validate_plan_proposal};
pub use types::{
    Decision, Finding, PlanProposal, PlanReviewer, PlanWalkthrough,
    ReviewError, ReviewEvent, ReviewInput, ReviewVerdict, ReviewedPlanDigest,
};
pub use verdict::{call_independent_model, emit_walkthrough, verdict_to_digest};

#[cfg(test)]
mod tests {
    use super::*;

    struct FakePlanReviewer { approve: bool }
    impl PlanReviewer for FakePlanReviewer {
        fn review(&self, input: &ReviewInput) -> Result<ReviewVerdict, ReviewError> {
            if input.reviewer_id == input.worker_id {
                return Err(ReviewError::NotIndependent);
            }
            Ok(ReviewVerdict {
                decision: if self.approve { Decision::Accept } else { Decision::Reject },
                findings: vec![],
                input_digest: "in1".into(),
                output_digest: "out1".into(),
                checked: 3,
                total: 3,
            })
        }
    }

    fn proposal(worker: &str) -> PlanProposal {
        PlanProposal { plan_digest: "pd1".into(), worker_id: worker.into(), evidence_digest: "ed1".into() }
    }

    #[test]
    fn proposal_reviewed_by_independent_model() {
        let reviewer = FakePlanReviewer { approve: true };
        let events = call_independent_model(&proposal("worker-1"), "reviewer-1", &reviewer).unwrap();
        let digest = events.iter().find_map(|e| if let ReviewEvent::Digest(d) = e { Some(d) } else { None });
        assert!(digest.is_some());
        assert_ne!(digest.unwrap().reviewer_id, "worker-1");
        assert!(digest.unwrap().checked > 0);
    }

    #[test]
    fn walkthrough_emitted_before_ready() {
        let reviewer = FakePlanReviewer { approve: true };
        let events = call_independent_model(&proposal("worker-1"), "reviewer-1", &reviewer).unwrap();
        let wt_pos = events.iter().position(|e| matches!(e, ReviewEvent::Walkthrough(_)));
        let dg_pos = events.iter().position(|e| matches!(e, ReviewEvent::Digest(_)));
        assert!(wt_pos.is_some() && dg_pos.is_some());
        assert!(wt_pos.unwrap() < dg_pos.unwrap());
    }

    #[test]
    fn review_rejection_blocks_ready() {
        let reviewer = FakePlanReviewer { approve: false };
        let events = call_independent_model(&proposal("worker-1"), "reviewer-1", &reviewer).unwrap();
        let digest = events.iter().find_map(|e| if let ReviewEvent::Digest(d) = e { Some(d) } else { None }).unwrap();
        assert!(!digest.approved);
    }
}
