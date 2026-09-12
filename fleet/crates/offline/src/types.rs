use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeldOutTask {
    pub id: String,
    pub input: String,
    pub input_digest: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    Baseline,
    Candidate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub task_id: String,
    pub variant: Variant,
    pub passed: bool,
}

pub struct EvaluationRequest {
    pub candidate_id: String,
    pub tasks: Vec<HeldOutTask>,
    pub min_pairs: u64,
    pub seed: u64,
}

pub type LessonCandidate = EvaluationRequest;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Recommendation {
    Promote,
    Retain,
    Reject,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfflineScore {
    pub candidate_id: String,
    pub baseline_pass: u64,
    pub variant_pass: u64,
    pub checked: u64,
    pub total: u64,
    pub recommendation: Recommendation,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatedLesson {
    pub candidate_id: String,
    pub score: OfflineScore,
}

#[derive(Debug, Error)]
pub enum OfflineError {
    #[error("pair mismatch: incomparable task digests")]
    PairMismatch,
    #[error("coverage failure: checked and total must both be > 0")]
    Coverage,
    #[error("runner unavailable: {0}")]
    Unavailable(String),
    #[error("insufficient pairs: need {need}, got {got}")]
    InsufficientPairs { need: u64, got: u64 },
}
