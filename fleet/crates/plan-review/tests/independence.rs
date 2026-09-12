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
        use std::sync::mpsc;

        struct FakePR { reviewer_id: String, approve: bool, checked: u64, total: u64 }

        impl pr::PlanReviewer for FakePR {
            fn review(&self, i: &pr::ReviewInput) -> Result<pr::ReviewVerdict, pr::ReviewError> {
                Ok(pr::ReviewVerdict {
                    decision: if self.approve { pr::Decision::Approved } else { pr::Decision::Rejected },
                    findings: vec![],
                    input_digest: i.plan_digest.clone(),
                    output_digest: format!("out-{}", self.reviewer_id),
                    checked: self.checked,
                    total: self.total,
                })
            }
        }

        fn fake(approve: bool) -> FakePR {
            FakePR { reviewer_id: "r1".into(), approve, checked: 3, total: 3 }
        }

        #[test]
        fn proposal_reviewed_by_independent_model() {
            let r = fake(true);
            let ok = pr::ReviewInput {
                plan_digest: "p".into(),
                reviewer_id: "r1".into(),
                worker_id: "w1".into(),
                evidence_digest: "e".into(),
            };
            let v = pr::call_independent_model(&ok, &r).unwrap();
            assert_ne!(ok.reviewer_id, ok.worker_id, "reviewer must differ from worker");
            assert!(v.checked > 0, "checked must be > 0");
            let bad = pr::ReviewInput { reviewer_id: "w1".into(), worker_id: "w1".into(), ..ok };
            assert_eq!(
                pr::call_independent_model(&bad, &r).unwrap_err(),
                pr::ReviewError::NotIndependent,
            );
        }

        #[test]
        fn walkthrough_emitted_before_ready() {
            let (tx, rx) = mpsc::channel();
            let d = pr::ReviewedPlanDigest {
                approved: true, reviewer_id: "r1".into(), plan_digest: "p".into(),
                checked: 3, total: 3,
            };
            pr::emit_walkthrough(&d, &tx).unwrap();
            let evs: Vec<_> = rx.try_iter().collect();
            let wt = evs.iter().position(|e| matches!(e, pr::ReviewEvent::Walkthrough(_))).unwrap();
            let rd = evs.iter().position(|e| matches!(e, pr::ReviewEvent::Ready)).unwrap();
            assert!(wt < rd, "walkthrough must precede ready signal");
        }
    }
}
