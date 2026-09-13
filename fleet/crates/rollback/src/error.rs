#[derive(Debug, thiserror::Error)]
pub enum RollbackError {
    #[error("artifact not found: {0}")]
    ArtifactNotFound(String),
    #[error("containment: {0}")]
    Containment(String),
    #[error("CAS mismatch: {0}")]
    CasMismatch(String),
    #[error("needs approval: {0}")]
    NeedsApproval(String),
    #[error("receipt error: {0}")]
    Receipt(String),
    #[error("needs reconciliation: {0}")]
    NeedsReconciliation(String),
}
