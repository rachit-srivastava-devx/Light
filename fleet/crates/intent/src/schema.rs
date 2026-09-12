#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct IntentSpec {
    pub kind: String,
    pub description: String,
    pub constraints: Vec<String>,
    pub authority: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum WorkflowSelection {
    FullSdlc,
    FastFix,
    QueryOnly,
}

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum IntentError {
    #[error("unknown kind: {0}")]
    UnknownKind(String),
    #[error("description is empty")]
    EmptyDescription,
    #[error("extra authority field rejected: {0}")]
    ExtraAuthorityField(String),
}

pub fn classify(spec: &IntentSpec) -> Result<WorkflowSelection, IntentError> {
    if spec.description.is_empty() {
        return Err(IntentError::EmptyDescription);
    }
    match spec.kind.as_str() {
        "feature" | "refactor" => Ok(WorkflowSelection::FullSdlc),
        "fix" => Ok(WorkflowSelection::FastFix),
        "query" => Ok(WorkflowSelection::QueryOnly),
        other => Err(IntentError::UnknownKind(other.to_string())),
    }
}
