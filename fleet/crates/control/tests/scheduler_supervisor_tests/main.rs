use control::scheduler::{cas_guard, ReadyHeap};
use control::supervisor::{commit_then_decide, validate_generation, SpawnDecision};
use control::{AuthorityStore, ControlError, Snapshot, TaskState};
use std::sync::Mutex;

struct NullStore {
    writes: Mutex<Vec<String>>,
}
impl NullStore {
    fn new() -> Self {
        Self {
            writes: Mutex::new(vec![]),
        }
    }
}
impl AuthorityStore for NullStore {
    fn write_receipt(&self, _: &str) -> Result<(), ControlError> {
        Ok(())
    }
    fn write_state(&self, id: &str) -> Result<(), ControlError> {
        self.writes.lock().unwrap().push(id.to_string());
        Ok(())
    }
    fn has_event(&self, _: &str) -> bool {
        false
    }
}

fn ps(rev: u64) -> Snapshot {
    Snapshot {
        task_id: "t".into(),
        state: TaskState::Pending,
        revision: rev,
    }
}

#[test]
fn heap_push_pop_order_by_score() {
    let mut h = ReadyHeap::new();
    h.push("low".into(), 1);
    h.push("high".into(), 10);
    h.push("mid".into(), 5);
    assert_eq!(h.pop(), Some("high".into()));
    assert_eq!(h.pop(), Some("mid".into()));
    assert_eq!(h.pop(), Some("low".into()));
    assert_eq!(h.pop(), None);
}

#[test]
fn heap_tiebreak_by_task_id() {
    let mut h = ReadyHeap::new();
    h.push("b".into(), 7);
    h.push("a".into(), 7);
    assert_eq!(h.pop(), Some("a".into()));
    assert_eq!(h.pop(), Some("b".into()));
}

#[test]
fn heap_len_and_is_empty() {
    let mut h = ReadyHeap::new();
    assert!(h.is_empty());
    assert_eq!(h.len(), 0);
    h.push("x".into(), 1);
    assert!(!h.is_empty());
    assert_eq!(h.len(), 1);
    h.pop();
    assert!(h.is_empty());
}

#[test]
fn cas_guard_matches_ok() {
    cas_guard(&ps(3), 3).unwrap();
}

#[test]
fn cas_guard_mismatch_errors() {
    assert!(matches!(
        cas_guard(&ps(3), 5).unwrap_err(),
        ControlError::Store(_)
    ));
}

#[test]
fn validate_generation_match_ok() {
    validate_generation(4, 4).unwrap();
}

#[test]
fn validate_generation_mismatch_errors() {
    assert!(matches!(
        validate_generation(3, 4).unwrap_err(),
        ControlError::Store(_)
    ));
}

#[test]
fn commit_then_decide_writes_state_before_spawn() {
    let store = NullStore::new();
    let d = commit_then_decide(&store, "t1", true).unwrap();
    assert!(matches!(d, SpawnDecision::Launch { .. }));
    assert_eq!(store.writes.lock().unwrap().as_slice(), &["t1"]);
}
