use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Immutable input to the planner.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlanInput {
    pub task_digest: String,
    pub recipe_digest: String,
    pub context_digest: String,
    pub acceptance_refs: Vec<String>,
    pub max_modules: u32,
}

/// A single module proposed by the planner model.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModuleDraft {
    pub id: String,
    pub dependencies: Vec<String>,
    pub write_set: Vec<String>,
    pub acceptance_refs: Vec<String>,
}

/// Model-proposed module plan.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlanDraft {
    pub version: u32,
    pub modules: Vec<ModuleDraft>,
    pub explanation: String,
}

/// Validation summary (checked / total).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidationReport {
    pub checked: u32,
    pub total: u32,
    pub digest: String,
}

#[derive(Clone, Debug, Error)]
pub enum PlannerError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("invalid draft: {0}")]
    InvalidDraft(String),
    #[error("schema or digest error: {0}")]
    SchemaOrDigest(String),
    #[error("provider unavailable: {0}")]
    ProviderUnavailable(String),
}
