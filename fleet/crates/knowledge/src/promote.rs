use crate::{KnowledgeError, KnowledgeItem};

/// Gate that enforces promotion requirements before a candidate is stored.
///
/// Rules:
/// - `evidence_count` must be at least 1; a zero-evidence candidate is refused
///   with [`KnowledgeError::InsufficientEvidence`].
pub fn promote_candidate(item: &KnowledgeItem) -> Result<(), KnowledgeError> {
    if item.evidence_count == 0 {
        return Err(KnowledgeError::InsufficientEvidence);
    }
    Ok(())
}
