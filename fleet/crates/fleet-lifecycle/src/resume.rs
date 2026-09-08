//! `resume`: the sanctioned re-entry point for a caller that only has a state NAME. New
//! surface area (§ divergence note in BLUEPRINT.md) -- the private-field construction trick
//! `lifecycle.rs`'s `typed_advance`/`propose_change` used cannot move to the caller because
//! `Task<S>`'s fields are private to this crate by design.

use crate::states::*;
use crate::task::Task;
use crate::task_id::TaskId;
use fleet_types::GateRefusal;
use std::marker::PhantomData;

/// Every state, erased to a single enum so a caller that only knows a task's state as a
/// persisted `&str` (loaded from disk) can get back into the typed world.
#[derive(Debug)]
pub enum AnyTask {
    Intake(Task<Intake>),
    Specified(Task<Specified>),
    Reviewed(Task<Reviewed>),
    Decomposed(Task<Decomposed>),
    Contracted(Task<Contracted>),
    Briefed(Task<Briefed>),
    Leased(Task<Leased>),
    Building(Task<Building>),
    Built(Task<Built>),
    Verifying(Task<Verifying>),
    Verified(Task<Verified>),
    Attested(Task<Attested>),
    Accepted(Task<Accepted>),
    Proposed(Task<Proposed>),
    Observed(Task<Observed>),
    Refused(Task<Refused>),
}

macro_rules! any {
    ($variant:ident, $id:expr, $retry_depth:expr) => {
        AnyTask::$variant(Task { id: $id, retry_depth: $retry_depth, _s: PhantomData })
    };
}

/// Reconstruct a task in the state named by `state` (one of the 16 wire names, e.g.
/// `"Built"`). Pure: no IO, just a match + the private constructor.
/// `Err(GateRefusal{code: "UNKNOWN_LIFECYCLE_STATE", ..})` for any other string.
pub fn resume(state: &str, id: TaskId, retry_depth: u32) -> Result<AnyTask, GateRefusal> {
    Ok(match state {
        "Intake" => any!(Intake, id, retry_depth),
        "Specified" => any!(Specified, id, retry_depth),
        "Reviewed" => any!(Reviewed, id, retry_depth),
        "Decomposed" => any!(Decomposed, id, retry_depth),
        "Contracted" => any!(Contracted, id, retry_depth),
        "Briefed" => any!(Briefed, id, retry_depth),
        "Leased" => any!(Leased, id, retry_depth),
        "Building" => any!(Building, id, retry_depth),
        "Built" => any!(Built, id, retry_depth),
        "Verifying" => any!(Verifying, id, retry_depth),
        "Verified" => any!(Verified, id, retry_depth),
        "Attested" => any!(Attested, id, retry_depth),
        "Accepted" => any!(Accepted, id, retry_depth),
        "Proposed" => any!(Proposed, id, retry_depth),
        "Observed" => any!(Observed, id, retry_depth),
        "Refused" => any!(Refused, id, retry_depth),
        other => {
            return Err(GateRefusal::new(
                "UNKNOWN_LIFECYCLE_STATE",
                format!("{other} is not one of the 16 lifecycle states"),
            ))
        }
    })
}
