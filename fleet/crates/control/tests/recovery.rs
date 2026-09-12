use control::{reduce, AuthorityStore, ControlError, ControlEvent, Snapshot, TaskState};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

struct MockStore {
    receipts: Mutex<Vec<String>>,
    state_writes: Mutex<HashMap<String, u32>>,
    seen: Mutex<HashSet<String>>,
}

impl MockStore {
    fn new() -> Self {
        Self {
            receipts: Mutex::new(Vec::new()),
            state_writes: Mutex::new(HashMap::new()),
            seen: Mutex::new(HashSet::new()),
        }
    }
}

impl AuthorityStore for MockStore {
    fn write_receipt(&self, id: &str) -> Result<(), ControlError> {
        self.receipts.lock().unwrap().push(id.to_string());
        if let Some(eid) = id.strip_prefix("receipt-") {
            self.seen.lock().unwrap().insert(eid.to_string());
        }
        Ok(())
    }
    fn write_state(&self, task_id: &str) -> Result<(), ControlError> {
        *self.state_writes.lock().unwrap().entry(task_id.to_string()).or_insert(0) += 1;
        Ok(())
    }
    fn has_event(&self, event_id: &str) -> bool {
        self.seen.lock().unwrap().contains(event_id)
    }
}

fn ev(id: &str, task: &str, from: TaskState, to: TaskState) -> ControlEvent {
    ControlEvent { event_id: id.to_string(), task_id: task.to_string(), from_state: from, to_state: to }
}

fn snap(task: &str, state: TaskState, rev: u64) -> Snapshot {
    Snapshot { task_id: task.to_string(), state, revision: rev }
}

#[test]
fn ingest_event_drives_state_then_intent() {
    let store = MockStore::new();
    let t = reduce(&snap("t1", TaskState::Pending, 0), ev("e1", "t1", TaskState::Pending, TaskState::Running), &store).unwrap();
    assert_eq!(t.new_state, TaskState::Running);
    assert_eq!(t.revision, 1);
    assert_eq!(t.intents.len(), 1);
    assert_eq!(t.intents[0].task_id, "t1");
    assert_eq!(*store.state_writes.lock().unwrap().get("t1").unwrap(), 1);
}

#[test]
fn route_refusal_persists_receipt() {
    let store = MockStore::new();
    let err = reduce(&snap("t2", TaskState::Completed, 2), ev("e2", "t2", TaskState::Completed, TaskState::Running), &store).unwrap_err();
    assert!(matches!(err, ControlError::IllegalTransition { .. }));
    assert!(store.receipts.lock().unwrap().contains(&"receipt-e2".to_string()));
    assert!(store.state_writes.lock().unwrap().is_empty());
}

#[test]
fn duplicate_event_idempotent() {
    let store = MockStore::new();
    let t1 = reduce(&snap("t3", TaskState::Pending, 0), ev("e3", "t3", TaskState::Pending, TaskState::Running), &store).unwrap();
    let snap2 = snap("t3", t1.new_state.clone(), t1.revision);
    let t2 = reduce(&snap2, ev("e3", "t3", TaskState::Pending, TaskState::Running), &store).unwrap();
    assert_eq!(t1.receipt_id, t2.receipt_id);
    assert_eq!(*store.state_writes.lock().unwrap().get("t3").unwrap(), 1);
}
