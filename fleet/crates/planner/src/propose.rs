use crate::types::{PlanDraft, PlanInput, PlannerError};

/// Injected model port. No direct model calls allowed (C9).
pub trait PlannerModel: Send + Sync {
    fn propose(&self, input: &PlanInput) -> Result<PlanDraft, PlannerError>;
}
