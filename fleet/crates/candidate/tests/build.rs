mod candidate {
    mod tests {
        use ::candidate::types::Status;
        use ::candidate::{build, CandidateError, CandidateStore, Evidence, InMemoryStore};

        fn make_evidence() -> Evidence {
            Evidence {
                task_type: "verify".to_string(),
                signature: "sig-abc".to_string(),
                tree_digest: "tree-xyz".to_string(),
                evidence_digest: "evd-xyz".to_string(),
                checked: 3,
                total: 3,
            }
        }

        #[test]
        fn verify_candidate_offline_contract() {
            let e = make_evidence();
            let tree = e.tree_digest.clone();
            let r = build(e, "fix-digest".to_string()).unwrap();
            assert_eq!(r.status, Status::Candidate);
            assert!(!r.fixture_digest.is_empty());
            assert!(r.provenance.contains(&tree));
        }

        #[test]
        fn zero_coverage_refused() {
            let e = Evidence {
                task_type: "verify".to_string(),
                signature: "sig".to_string(),
                tree_digest: "tree".to_string(),
                evidence_digest: "evd".to_string(),
                checked: 0,
                total: 0,
            };
            let r = build(e, "fix".to_string());
            assert!(matches!(r, Err(CandidateError::Coverage)));
        }

        #[test]
        fn duplicate_candidate_conflicts() {
            let e = make_evidence();
            let lesson = build(e.clone(), "fix-digest".to_string()).unwrap();
            let mut store = InMemoryStore::new();
            assert!(store.insert(&lesson).is_ok());
            assert!(matches!(
                store.insert(&lesson),
                Err(CandidateError::Conflict)
            ));
        }
    }
}
