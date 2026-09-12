pub extern crate plan_review as pr;

mod plan_review {
    pub use super::pr;

    pub mod tests {
        use super::pr;
        use std::sync::mpsc;

        #[test]
        fn review_rejection_blocks_ready() {
            let (tx, rx) = mpsc::channel();
            let d = pr::ReviewedPlanDigest {
                approved: false,
                reviewer_id: "r1".into(),
                plan_digest: "p".into(),
                checked: 2,
                total: 3,
            };
            pr::emit_walkthrough(&d, &tx).unwrap();
            let evs: Vec<_> = rx.try_iter().collect();
            assert!(
                !evs.iter().any(|e| matches!(e, pr::ReviewEvent::Ready)),
                "ready must be blocked on rejection",
            );
        }

        #[test]
        fn reviewed_plan_digest_fields_populated() {
            let (tx, rx) = mpsc::channel();
            let d = pr::ReviewedPlanDigest {
                approved: true,
                reviewer_id: "opus".into(),
                plan_digest: "abc".into(),
                checked: 1,
                total: 1,
            };
            pr::emit_walkthrough(&d, &tx).unwrap();
            let evs: Vec<_> = rx.try_iter().collect();
            assert_eq!(evs.len(), 2, "approved digest must emit exactly 2 events");
            let wt = match &evs[0] {
                pr::ReviewEvent::Walkthrough(w) => w.clone(),
                _ => panic!("first event must be Walkthrough"),
            };
            assert_eq!(wt.plan_digest, "abc", "walkthrough plan_digest mismatch");
            assert_eq!(wt.reviewer_id, "opus", "walkthrough reviewer_id mismatch");
            assert!(matches!(evs[1], pr::ReviewEvent::Ready), "second event must be Ready");
        }

        #[test]
        fn rejected_review_blocks_ready() {
            let (tx, rx) = mpsc::channel();
            let d = pr::ReviewedPlanDigest {
                approved: false,
                reviewer_id: "r1".into(),
                plan_digest: "p".into(),
                checked: 1,
                total: 1,
            };
            pr::emit_walkthrough(&d, &tx).unwrap();
            let evs: Vec<_> = rx.try_iter().collect();
            assert_eq!(evs.len(), 1, "rejected digest must emit exactly 1 event");
            assert!(matches!(evs[0], pr::ReviewEvent::Walkthrough(_)), "only event must be Walkthrough");
        }

    }
}
