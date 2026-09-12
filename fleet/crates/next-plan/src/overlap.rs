use crate::queue::NextQueue;
use crate::types::{NextError, NextInput, NextPlanSignal, NextProposal};

/// Returns `true` when no candidate module appears in N's write or measure sets.
fn sets_disjoint(
    current_write: &[String],
    current_measure: &[String],
    candidate_modules: &[String],
) -> bool {
    !candidate_modules
        .iter()
        .any(|m| current_write.contains(m) || current_measure.contains(m))
}

/// Propose the next (N+1) plan given the current plan context and a candidate draft.
///
/// Errors:
/// - `Stale` — `current_plan_digest` is empty (no immutable N exists).
/// - `Overlap` — candidate modules intersect N's write/measure sets.
/// - `Backpressure` — queue is at capacity.
pub fn propose_next(
    input: &NextInput,
    queue: &mut dyn NextQueue,
) -> Result<NextProposal, NextError> {
    if input.current_plan_digest.is_empty() {
        return Err(NextError::Stale);
    }
    let disjoint = sets_disjoint(
        &input.current_write_set,
        &input.current_measure_set,
        &input.candidate.modules,
    );
    if !disjoint {
        return Err(NextError::Overlap);
    }
    let cursor = queue.next_cursor();
    queue.push(NextPlanSignal {
        parent_digest: input.current_plan_digest.clone(),
        candidate: input.candidate.clone(),
        write_set: vec![],
        measure_set: vec![],
    })?;
    Ok(NextProposal {
        plan: input.candidate.clone(),
        disjoint: true,
        parent_digest: input.current_plan_digest.clone(),
        cursor,
    })
}
