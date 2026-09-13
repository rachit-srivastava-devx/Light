pub mod gate;
pub mod prompt;
pub mod schema;

pub use gate::independent_kind;
pub use prompt::IntentModel;
pub use schema::validate;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Effect {
    pub kind: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct IntentInput {
    pub request: String,
    pub context: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PolicySnapshot {
    pub allowed_effects: Vec<Effect>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ModelProposal {
    pub kind: String,
    pub goal: String,
    pub effects: Vec<Effect>,
    pub evidence: Vec<String>,
    pub raw_json: serde_json::Value,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct IntentSpec {
    pub kind: String,
    pub goal: String,
    pub effects: Vec<Effect>,
    pub evidence: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum IntentError {
    #[error("unknown workflow kind: {0}")]
    UnknownKind(String),
    #[error("authority field rejected: {0}")]
    AuthorityField(String),
    #[error("missing evidence for: {0}")]
    MissingEvidence(String),
    #[error("goal is empty")]
    EmptyGoal,
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
