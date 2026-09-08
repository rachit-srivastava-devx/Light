//! Typed errors for the plan-ahead overlap orchestrator (`orchestrator.rs`). No `.unwrap()`s or
//! opaque `String` catch-alls hide a real cause here (BLUEPRINT §11 discipline, same as
//! `dispatch/error.rs`).

#[derive(Debug, thiserror::Error)]
pub enum PlanAheadError {
    /// A unit's `plan_fn` returned an error; `unit` and the underlying message are both kept.
    #[error("planning unit {unit:?} failed: {message}")]
    Plan { unit: String, message: String },
    /// A unit's `build_fn` returned an error.
    #[error("building unit {unit:?} failed: {message}")]
    Build { unit: String, message: String },
    /// The build side hung up (its task panicked or was cancelled) while the planner still had
    /// units to send -- the planner must not silently drop the rest of the run.
    #[error("the build_q receiver was dropped while units were still queued to plan")]
    BuildSideGone,
    /// A planned/built marker could not be persisted to `UnitLog`'s backing file.
    #[error("unit log io failed: {0}")]
    Io(#[from] std::io::Error),
    /// A spawned tokio task panicked instead of returning a value.
    #[error("a plan-ahead worker task panicked: {0}")]
    WorkerPanicked(String),
}
