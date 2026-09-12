// Integration tests — named tests must appear verbatim (§10).
// `mod plan_review { pub mod tests { ... } }` produces the DoD test paths:
//   test plan_review::tests::<name> ... ok
//
// External crate aliased as `pr` to avoid shadowing by local mod plan_review.
pub extern crate plan_review as pr;

mod plan_review {
    pub use super::pr;

    pub mod tests {
        use super::pr;

        struct FakePR { approve: bool }

        impl pr::PlanReviewer for FakePR {
            fn review(&self, i: &pr::ReviewInput) -> Result<pr::ReviewVerdict, pr::ReviewError> {
                Ok(pr::ReviewVerdict {
                    decision: if self.approve { pr::Decision::Accept } else { pr::Decision::Reject },
                    findings: vec![],
                    input_digest: i.plan_digest.clone(),
                    output_digest: format!("out-{}", i.reviewer_id),
                    checked: 3,
                    total: 3,
                })
            }
        }

        fn proposal(worker: &str) -> pr::PlanProposal {
            pr::PlanProposal {
                plan_digest: "p".into(),
                worker_id: worker.into(),
                evidence_digest: "e".into(),
            }
        }

        #[test]
        fn proposal_reviewed_by_independent_model() {
            let r = FakePR { approve: true };
            let evs = pr::call_independent_model(&proposal("w1"), "r1", &r).unwrap();
            let digest = evs.iter().find_map(|e| {
                if let pr::ReviewEvent::Digest(d) = e { Some(d) } else { None }
            }).unwrap();
            assert_ne!(digest.reviewer_id, "w1", "reviewer must differ from worker");
            assert!(digest.checked > 0, "checked must be > 0");
            // same reviewer_id == worker_id must be rejected by the independence gate
            let err = pr::call_independent_model(&proposal("r1"), "r1", &r).unwrap_err();
            assert!(matches!(err, pr::ReviewError::NotIndependent));
        }

        #[test]
        fn walkthrough_emitted_before_ready() {
            let r = FakePR { approve: true };
            let evs = pr::call_independent_model(&proposal("w1"), "r1", &r).unwrap();
            let wt = evs.iter().position(|e| matches!(e, pr::ReviewEvent::Walkthrough(_))).unwrap();
            let rd = evs.iter().position(|e| matches!(e, pr::ReviewEvent::Digest(_))).unwrap();
            assert!(wt < rd, "walkthrough must precede ready signal");
        }
    }
}
