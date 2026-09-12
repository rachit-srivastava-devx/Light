//! Controller — lifecycle state machine and admission control.
//! Re-exports fleet-lifecycle and fleet-govern for backward compat while new implementation matures.
pub use fleet_govern::*;
pub use fleet_lifecycle::*;

// ── Core types used by the sub-modules ─────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TaskState {
    Pending,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub task_id: String,
    pub state: TaskState,
    pub revision: u64,
}

#[derive(Debug, Clone)]
pub struct ControlEvent {
    pub event_id: String,
    pub task_id: String,
    pub from_state: TaskState,
    pub to_state: TaskState,
}

#[derive(Debug, Clone)]
pub struct IntentSpec {
    pub task_id: String,
}

#[derive(Debug, Clone)]
pub struct Transition {
    pub new_state: TaskState,
    pub revision: u64,
    pub intents: Vec<IntentSpec>,
    pub receipt_id: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ControlError {
    #[error("state mismatch: snapshot={snapshot:?} event={event:?}")]
    StateMismatch { snapshot: TaskState, event: TaskState },
    #[error("illegal transition: {from:?} → {to:?}")]
    IllegalTransition { from: TaskState, to: TaskState },
    #[error("store error: {0}")]
    Store(String),
}

pub trait AuthorityStore {
    fn has_event(&self, event_id: &str) -> bool;
    fn write_receipt(&self, receipt_id: &str) -> Result<(), ControlError>;
    fn write_state(&self, task_id: &str) -> Result<(), ControlError>;
}

// ── Sub-modules ─────────────────────────────────────────────────────────────

pub mod reducer;
pub mod scheduler;
pub mod supervisor;

pub use reducer::reduce;
pub use scheduler::{cas_guard, ReadyHeap};
pub use supervisor::{commit_then_decide, validate_generation, SpawnDecision};
