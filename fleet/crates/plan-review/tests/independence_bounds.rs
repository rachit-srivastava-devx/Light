pub extern crate plan_review as pr;

mod plan_review {
    pub use super::pr;

    pub mod tests {
        use super::pr;

        fn proposal(plan: &str, worker: &str) -> pr::PlanProposal {
            pr::PlanProposal {
                plan_digest: plan.into(),
                worker_id: worker.into(),
                evidence_digest: "e".into(),
            }
        }

        fn verdict(decision: pr::Decision, checked: u64, total: u64) -> pr::ReviewVerdict {
            pr::ReviewVerdict {
                decision,
                findings: vec![],
                input_digest: "in1".into(),
                output_digest: "out1".into(),
                checked,
                total,
            }
        }

        fn get_digest(evs: &[pr::ReviewEvent]) -> &pr::ReviewedPlanDigest {
            evs.iter().find_map(|e| {
                if let pr::ReviewEvent::Digest(d) = e { Some(d) } else { None }
            }).unwrap()
        }

        #[test]
        fn review_rejection_blocks_ready() {
            let p = proposal("p", "w1");
            let v = verdict(pr::Decision::Reject, 2, 3);
            let evs = pr::emit_walkthrough(&p, "r1", &v);
            let d = get_digest(&evs);
            assert!(!d.approved, "ready must be blocked on rejection");
        }

        #[test]
        fn reviewed_plan_digest_fields_populated() {
            let p = proposal("abc", "w1");
            let v = verdict(pr::Decision::Accept, 1, 1);
            let evs = pr::emit_walkthrough(&p, "opus", &v);
            assert_eq!(evs.len(), 2, "approved digest must emit exactly 2 events");
            let wt = match &evs[0] {
                pr::ReviewEvent::Walkthrough(w) => w.clone(),
                _ => panic!("first event must be Walkthrough"),
            };
            assert_eq!(wt.plan_digest, "abc", "walkthrough plan_digest mismatch");
            assert_eq!(wt.reviewer_id, "opus", "walkthrough reviewer_id mismatch");
            let d = get_digest(&evs);
            assert!(d.approved, "Digest must be approved on acceptance");
        }

        #[test]
        fn rejected_review_blocks_ready() {
            let p = proposal("p", "w1");
            let v = verdict(pr::Decision::Reject, 1, 1);
            let evs = pr::emit_walkthrough(&p, "r1", &v);
            assert!(matches!(evs[0], pr::ReviewEvent::Walkthrough(_)), "first event must be Walkthrough");
            let d = get_digest(&evs);
            assert!(!d.approved, "Digest must not be approved on rejection");
        }
    }
}
