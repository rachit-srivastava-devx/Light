mod approval {
    mod tests {
        use ::approval::{approve, ApprovalError, ApprovalGrant, ApprovalRequest, ApprovalStore};
        use std::collections::{HashMap, HashSet};

        struct MockStore {
            grants: HashMap<String, ApprovalGrant>,
            consumed: HashSet<String>,
        }

        impl MockStore {
            fn new() -> Self {
                Self {
                    grants: HashMap::new(),
                    consumed: HashSet::new(),
                }
            }
        }

        impl ApprovalStore for MockStore {
            fn put(&mut self, grant: &ApprovalGrant) -> Result<(), ApprovalError> {
                self.grants.insert(grant.approval_id.clone(), grant.clone());
                Ok(())
            }

            fn consume(&mut self, id: &str) -> Result<ApprovalGrant, ApprovalError> {
                if self.consumed.contains(id) {
                    return Err(ApprovalError::Replay);
                }
                let grant = self
                    .grants
                    .get(id)
                    .cloned()
                    .ok_or_else(|| ApprovalError::NotFound(id.to_string()))?;
                self.consumed.insert(id.to_string());
                Ok(grant)
            }
        }

        fn make_req(expires_at: u64) -> ApprovalRequest {
            ApprovalRequest {
                task_id: "task-1".to_string(),
                action: "publish".to_string(),
                resource: "main".to_string(),
                scope_hash: "deadbeef".to_string(),
                base_commit: "abc123".to_string(),
                artifact_id: "art-42".to_string(),
                expires_at,
            }
        }

        #[test]
        fn grant_consumed_on_approval() {
            let mut store = MockStore::new();
            let grant = approve(&mut store, make_req(1000), "operator".to_string(), 1).unwrap();
            assert_eq!(
                store.grants.len(),
                1,
                "store must contain exactly one record"
            );
            let stored = &store.grants[&grant.approval_id];
            assert_eq!(stored.request.scope_hash, "deadbeef");
            assert_eq!(stored.actor, "operator");
        }

        #[test]
        fn replay_refused_after_consume() {
            let mut store = MockStore::new();
            let grant = approve(&mut store, make_req(1000), "operator".to_string(), 1).unwrap();
            let id = grant.approval_id.clone();
            let first = store.consume(&id);
            assert!(first.is_ok(), "first consume must succeed");
            let second = store.consume(&id);
            assert_eq!(
                second,
                Err(ApprovalError::Replay),
                "second consume must be Replay"
            );
        }

        #[test]
        fn expired_grant_refused() {
            let mut store = MockStore::new();
            // expires_at=5, now=10 → expired
            let result = approve(&mut store, make_req(5), "operator".to_string(), 10);
            assert_eq!(result, Err(ApprovalError::Expired));
            assert!(
                store.grants.is_empty(),
                "no store write must occur on expiry"
            );
        }
    }
}
