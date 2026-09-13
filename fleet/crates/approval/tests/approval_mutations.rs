use approval::{approve, consume, ApprovalError, ApprovalGrant, ApprovalRequest, ApprovalStore};
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
    fn put(&mut self, g: &ApprovalGrant) -> Result<(), ApprovalError> {
        self.grants.insert(g.approval_id.clone(), g.clone());
        Ok(())
    }
    fn consume(&mut self, id: &str) -> Result<ApprovalGrant, ApprovalError> {
        if self.consumed.contains(id) {
            return Err(ApprovalError::Replay);
        }
        let g = self
            .grants
            .get(id)
            .cloned()
            .ok_or_else(|| ApprovalError::NotFound(id.to_string()))?;
        self.consumed.insert(id.to_string());
        Ok(g)
    }
}

fn make_req(expires: u64) -> ApprovalRequest {
    ApprovalRequest {
        task_id: "task-1".into(),
        action: "publish".into(),
        resource: "main".into(),
        scope_hash: "deadbeef".into(),
        base_commit: "abc123".into(),
        artifact_id: "art-42".into(),
        expires_at: expires,
    }
}
#[test]
fn approval_id_exact_format() {
    let mut store = MockStore::new();
    let grant = approve(&mut store, make_req(1000), "op".into(), 5).unwrap();
    assert_eq!(grant.approval_id, "grant-task-1-5");
}
#[test]
fn issued_at_exact_value() {
    let mut store = MockStore::new();
    let grant = approve(&mut store, make_req(1000), "op".into(), 42).unwrap();
    assert_eq!(grant.issued_at, 42);
}
#[test]
fn empty_actor_refused_with_correct_field_name() {
    let mut store = MockStore::new();
    let err = approve(&mut store, make_req(1000), String::new(), 1).unwrap_err();
    assert_eq!(err, ApprovalError::Empty("actor".to_string()));
}
#[test]
fn empty_task_id_refused() {
    let mut store = MockStore::new();
    let mut req = make_req(1000);
    req.task_id = String::new();
    let err = approve(&mut store, req, "op".into(), 1).unwrap_err();
    assert_eq!(err, ApprovalError::Empty("task_id".to_string()));
}
#[test]
fn consume_expired_grant_refused() {
    let mut store = MockStore::new();
    let grant = approve(&mut store, make_req(10), "op".into(), 1).unwrap();
    let err = consume(&mut store, &grant.approval_id, 20).unwrap_err();
    assert_eq!(err, ApprovalError::Expired);
}
#[test]
fn consume_unknown_id_returns_not_found() {
    let mut store = MockStore::new();
    let err = consume(&mut store, "no-such-id", 1).unwrap_err();
    assert!(matches!(err, ApprovalError::NotFound(_)));
}
#[test]
fn grant_request_fields_preserved_exactly() {
    let mut store = MockStore::new();
    let grant = approve(&mut store, make_req(1000), "op".into(), 1).unwrap();
    assert_eq!(grant.request.task_id, "task-1");
    assert_eq!(grant.request.scope_hash, "deadbeef");
    assert_eq!(grant.actor, "op");
}
