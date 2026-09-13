use crate::{
    AuthorityStore, ControlError, ControlEvent, IntentSpec, Snapshot, TaskState, Transition,
};

fn is_legal(from: &TaskState, to: &TaskState) -> bool {
    matches!(
        (from, to),
        (TaskState::Pending, TaskState::Running)
            | (TaskState::Running, TaskState::Completed)
            | (TaskState::Running, TaskState::Failed)
    )
}

/// Pure state-machine reducer with durable store writes.
///
/// Order of checks:
/// 1. Idempotency — if the event was already processed, return the same Ok.
/// 2. State mismatch — snapshot.state must equal event.from_state (prevents
///    callers from lying about current state, e.g. restarting a Completed task).
/// 3. Legal transition — illegal transitions write a refusal receipt and error.
/// 4. Commit — write receipt + state, then return Ok with emitted intents.
pub fn reduce(
    snapshot: &Snapshot,
    event: ControlEvent,
    store: &dyn AuthorityStore,
) -> Result<Transition, ControlError> {
    let receipt_id = format!("receipt-{}", event.event_id);

    // 1. Idempotency: event already committed — return stable Ok without
    //    re-writing state, preserving the monotonic-revision invariant.
    if store.has_event(&event.event_id) {
        return Ok(Transition {
            new_state: snapshot.state.clone(),
            revision: snapshot.revision,
            intents: vec![],
            receipt_id,
        });
    }

    // 2. State mismatch: caller's from_state must match the actual snapshot.
    //    A mismatch means the caller is lying (or stale) — reject immediately.
    if snapshot.state != event.from_state {
        return Err(ControlError::StateMismatch {
            snapshot: snapshot.state.clone(),
            event: event.from_state,
        });
    }

    // 3. Illegal transition: write refusal receipt so the caller has evidence,
    //    but do not advance state.
    if !is_legal(&event.from_state, &event.to_state) {
        store.write_receipt(&receipt_id)?;
        return Err(ControlError::IllegalTransition {
            from: event.from_state,
            to: event.to_state,
        });
    }

    // 4. Legal transition: commit receipt + state, then emit intents.
    let intents = if event.to_state == TaskState::Running {
        vec![IntentSpec {
            task_id: event.task_id.clone(),
        }]
    } else {
        vec![]
    };
    store.write_receipt(&receipt_id)?;
    store.write_state(&event.task_id)?;
    Ok(Transition {
        new_state: event.to_state,
        revision: snapshot.revision + 1,
        intents,
        receipt_id,
    })
}
