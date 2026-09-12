use crate::{IntentError, IntentInput, ModelProposal};

/// Port trait for model-backed intent proposal.
/// Callers inject an implementation; this crate never implements it.
pub trait IntentModel {
    fn propose(&mut self, input: IntentInput) -> Result<ModelProposal, IntentError>;
}
