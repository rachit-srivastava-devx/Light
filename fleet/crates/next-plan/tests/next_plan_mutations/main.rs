use next_plan::{
    propose_next, MemQueue, NextError, NextInput, NextPlanSignal, NextQueue, PlanDraft,
};

fn inp(digest: &str, write: &[&str], meas: &[&str], mods: &[&str]) -> NextInput {
    NextInput {
        current_plan_digest: digest.into(),
        current_write_set: write.iter().map(|s| s.to_string()).collect(),
        current_measure_set: meas.iter().map(|s| s.to_string()).collect(),
        candidate: PlanDraft {
            modules: mods.iter().map(|s| s.to_string()).collect(),
            digest: "x".into(),
        },
        queue_capacity: 4,
    }
}

fn sig(parent: &str) -> NextPlanSignal {
    NextPlanSignal {
        parent_digest: parent.into(),
        candidate: PlanDraft {
            modules: vec![],
            digest: parent.into(),
        },
        write_set: vec![],
        measure_set: vec![],
    }
}
#[test]
fn propose_overlap_write_set_returns_err() {
    let mut q = MemQueue::new(4);
    let err = propose_next(&inp("d", &["mod-a"], &[], &["mod-a"]), &mut q).unwrap_err();
    assert_eq!(err, NextError::Overlap);
}
#[test]
fn propose_overlap_measure_set_returns_err() {
    let mut q = MemQueue::new(4);
    let err = propose_next(&inp("d", &[], &["mod-a"], &["mod-a"]), &mut q).unwrap_err();
    assert_eq!(err, NextError::Overlap);
}
#[test]
fn propose_stale_when_digest_empty() {
    let mut q = MemQueue::new(4);
    let err = propose_next(&inp("", &[], &[], &["m"]), &mut q).unwrap_err();
    assert_eq!(err, NextError::Stale);
}
#[test]
fn propose_success_has_correct_fields() {
    let mut q = MemQueue::new(4);
    let p = propose_next(&inp("parent-d", &["x"], &["y"], &["z"]), &mut q).unwrap();
    assert!(p.disjoint, "disjoint field must be true");
    assert_eq!(p.parent_digest, "parent-d");
    assert_eq!(p.cursor, 1, "first cursor must be 1");
}
#[test]
fn next_cursor_starts_at_one() {
    let mut q = MemQueue::new(4);
    assert_eq!(q.next_cursor(), 1);
}
#[test]
fn next_cursor_increments_monotonically() {
    let mut q = MemQueue::new(4);
    let a = q.next_cursor();
    let b = q.next_cursor();
    let c = q.next_cursor();
    assert_eq!(a, 1);
    assert_eq!(b, 2);
    assert_eq!(c, 3);
}
#[test]
fn push_returns_true_new_false_duplicate() {
    let mut q = MemQueue::new(4);
    assert_eq!(q.push(sig("p1")), Ok(true));
    assert_eq!(q.push(sig("p1")), Ok(false));
}
#[test]
fn push_backpressure_exactly_at_capacity() {
    let mut q = MemQueue::new(2);
    q.push(sig("a")).unwrap();
    q.push(sig("b")).unwrap();
    assert_eq!(q.push(sig("c")), Err(NextError::Backpressure));
}
