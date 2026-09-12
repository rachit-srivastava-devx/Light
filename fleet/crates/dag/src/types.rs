use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub depends_on: Vec<String>,
    pub read_set: Vec<String>,
    pub write_set: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphVersion {
    pub id: String,
    pub revision: u64,
    pub nodes: Vec<Node>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowRecipe {
    pub id: String,
    pub version: u64,
    pub nodes: Vec<String>,
    pub edges: Vec<String>,
    pub digest: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SkipPlanningSignal {
    pub run_id: String,
    pub plan_digest: String,
    pub reason: String,
    pub checked: u64,
    pub total: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReadySet {
    pub revision: u64,
    pub ids: Vec<String>,
    pub checked: u64,
    pub total: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum DagError {
    #[error("graph is empty")]
    Empty,
    #[error("duplicate node id: {0}")]
    Duplicate(String),
    #[error("missing dependency: {0}")]
    MissingDependency(String),
    #[error("cycle detected in graph")]
    Cycle,
    #[error("stale revision")]
    StaleRevision,
}
