use crate::types::{CandidateError, CandidateLesson, Evidence, Status};

/// Deterministically construct a [`CandidateLesson`] from verified evidence.
///
/// Preconditions (all must hold or a typed refusal is returned):
/// - `checked > 0` and `checked == total` (coverage gate)
/// - all string fields nonempty
/// - `fixture_digest` nonempty
pub fn build(e: Evidence, fixture_digest: String) -> Result<CandidateLesson, CandidateError> {
    if e.checked == 0 || e.checked != e.total {
        return Err(CandidateError::Coverage);
    }
    if e.task_type.trim().is_empty()
        || e.signature.trim().is_empty()
        || e.tree_digest.trim().is_empty()
        || e.evidence_digest.trim().is_empty()
    {
        return Err(CandidateError::Invalid);
    }
    if fixture_digest.trim().is_empty() {
        return Err(CandidateError::Invalid);
    }
    let id = format!("{}:{}:{}", e.task_type, e.tree_digest, e.evidence_digest);
    let scope = e.task_type.clone();
    let provenance = vec![e.tree_digest.clone()];
    Ok(CandidateLesson {
        id,
        scope,
        fixture_digest,
        provenance,
        status: Status::Candidate,
    })
}
