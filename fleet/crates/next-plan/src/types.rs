use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanDraft {
    pub modules: Vec<String>,
    pub digest: String,
}

#[derive(Debug, Clone)]
pub struct NextPlanSignal {
    pub parent_digest: String,
    pub candidate: PlanDraft,
    pub write_set: Vec<String>,
    pub measure_set: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextInput {
    pub current_plan_digest: String,
    pub current_write_set: Vec<String>,
    pub current_measure_set: Vec<String>,
    pub candidate: PlanDraft,
    pub queue_capacity: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextProposal {
    pub plan: PlanDraft,
    pub disjoint: bool,
    pub parent_digest: String,
    pub cursor: u64,
}

#[derive(Debug, Error, PartialEq)]
pub enum NextError {
    #[error("write/measure sets overlap with current plan")]
    Overlap,
    #[error("stale parent digest — current plan has changed")]
    Stale,
    #[error("proposal queue at capacity")]
    Backpressure,
    #[error("corrupt resume log — unit already completed")]
    CorruptLog,
}
