//! End-to-end integration tests for the next-plan pipeline.
use next_plan::{emit_signal, MemQueue, NextPlanSignal, NextQueue, PlanDraft};

fn make_signal(parent: &str, module: &str) -> NextPlanSignal {
    NextPlanSignal {
        parent_digest: parent.into(),
        candidate: PlanDraft {
            modules: vec![module.into()],
            digest: format!("{parent}-{module}"),
        },
        write_set: vec![format!("{module}.rs")],
        measure_set: vec![format!("{module}_latency_ms")],
    }
}

#[tokio::test]
async fn resume_signal_accepted_and_queued() {
    let mut q = MemQueue::new(8);
    emit_signal(make_signal("plan-a", "modX"), &mut q).unwrap();
    assert_eq!(q.depth(), 1);
}

#[tokio::test]
async fn resume_second_signal_same_parent_is_deduplicated() {
    let mut q = MemQueue::new(8);
    emit_signal(make_signal("plan-b", "modY"), &mut q).unwrap();
    emit_signal(make_signal("plan-b", "modZ"), &mut q).unwrap();
    assert_eq!(q.depth(), 1, "same parent_digest must be deduplicated");
}

#[tokio::test]
async fn resume_empty_parent_never_queues() {
    let mut q = MemQueue::new(8);
    emit_signal(make_signal("", "modA"), &mut q).unwrap();
    assert_eq!(q.depth(), 0, "empty parent_digest is a noop");
}
