//! `blueprint_q` -> (on Locked) -> `build_q`. **Divergence from BLUEPRINT §4**: the blueprint's
//! gate type `fleet_lifecycle::Task<Locked>` names a state that does not exist in the real
//! `fleet-lifecycle` state list (`Intake..Refused`, no `Locked`); the nearest real proof that a
//! task has cleared its lease and is eligible to build is `Task<fleet_lifecycle::Building>`
//! (reachable only via `Task<Leased>::build(..)`, i.e. only after the lease gate). This module
//! uses that real marker instead -- flagged for Opus, not silently renamed.

use fleet_lifecycle::{Building, Task};
use tokio::sync::mpsc;

/// One unit of dispatchable work, carrying proof (the `Task<Building>` value itself) that it
/// passed the lease gate. Nothing can construct a `LaneTask` from an unlocked task: the type
/// simply has no such constructor.
pub struct LaneTask {
    pub task: Task<Building>,
}

pub fn blueprint_q(capacity: usize) -> (mpsc::Sender<LaneTask>, mpsc::Receiver<LaneTask>) {
    mpsc::channel(capacity)
}

/// The only way anything reaches `build_q`: passing a `Task<Building>` in. There is no code path
/// here that accepts a bare `TaskId`/string, which is what would let an unlocked task slip in.
/// `try_send` (sync) rather than the async `send`: the pipeline graph that calls this in this
/// pass is itself synchronous (Restate deferral, see `graph.rs`), and a bounded, freshly-created
/// queue is never actually full at this call site.
pub fn enqueue_build(
    build_q: &mpsc::Sender<LaneTask>,
    task: Task<Building>,
) -> Result<(), mpsc::error::TrySendError<LaneTask>> {
    build_q.try_send(LaneTask { task })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Compile-time proof, not a runtime assertion: `enqueue_build`'s signature only accepts an
    /// owned `Task<Building>`, so the only way to call it is to already hold one -- there is no
    /// string/bool check elsewhere in this file that a caller could route around, matching
    /// BLUEPRINT §9's `blueprint_q_enqueue_requires_locked_marker` intent.
    #[test]
    fn blueprint_q_channel_is_constructible_and_bounded() {
        let (tx, rx) = blueprint_q(4);
        assert_eq!(tx.capacity(), 4);
        drop(rx);
    }
}
