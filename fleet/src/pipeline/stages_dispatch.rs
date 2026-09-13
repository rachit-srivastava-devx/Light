//! The `Dispatch` stage's wiring, split out of `stages.rs` to stay under the 80-line file gate.
//! Runs `route::decide`, then proves the task cleared the lease gate and pushes it
//! through the real `blueprint_q` -> `build_q` channel pair (`channels.rs`) as a `Task<Building>`
//! -- the only value shape that channel accepts, per BLUEPRINT §4's Locked-gate invariant.

use super::channels::{blueprint_q, enqueue_build};
use super::event::PipelineError;
use types::Role;

pub fn dispatch(
    runtime: &route::RuntimeState,
    task_id: &types::TaskId,
) -> Result<(), PipelineError> {
    let decision = route::decide(
        Some(Role::Builder),
        route::TaskClass::General,
        None,
        runtime,
    );
    if let Some(r) = decision.refusal {
        return Err(PipelineError::Dispatch(r));
    }
    let lifecycle_id = control::TaskId::new(task_id.as_str())
        .map_err(|e| PipelineError::Runtime(e.message().to_string()))?;
    let any = control::resume("Building", lifecycle_id, 0)
        .map_err(|e| PipelineError::Runtime(e.message().to_string()))?;
    let control::AnyTask::Building(building) = any else {
        return Err(PipelineError::Runtime(
            "resume(\"Building\") yielded the wrong state".into(),
        ));
    };
    let (tx, mut rx) = blueprint_q(1);
    enqueue_build(&tx, building)
        .map_err(|_| PipelineError::Runtime("build_q enqueue failed".into()))?;
    let received = rx
        .try_recv()
        .map_err(|_| PipelineError::Runtime("build_q receive failed".into()))?;
    if received.task.id().as_str() != task_id.as_str() {
        return Err(PipelineError::Runtime(
            "build_q delivered the wrong task".into(),
        ));
    }
    Ok(())
}
