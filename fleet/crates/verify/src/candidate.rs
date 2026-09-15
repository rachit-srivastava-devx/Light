use crate::{ReviewedCandidate, VerifyError};
use std::collections::HashSet;

pub(crate) fn validate_candidate(candidate: &ReviewedCandidate) -> Result<(), VerifyError> {
    if candidate.gates.is_empty() {
        return Err(VerifyError::NoGates);
    }
    if candidate.tree_digest.trim().is_empty() || candidate.acceptance_digest.trim().is_empty() {
        return Err(VerifyError::InvalidCandidate(
            "tree and acceptance digests are required".into(),
        ));
    }
    if candidate.coverage_floor.is_some_and(|floor| floor > 100) {
        return Err(VerifyError::InvalidCandidate(
            "coverage floor must be between 0 and 100".into(),
        ));
    }
    let mut seen = HashSet::new();
    for gate in &candidate.gates {
        if gate.id.trim().is_empty() || gate.command.trim().is_empty() {
            return Err(VerifyError::InvalidGate(
                "gate id and command are required".into(),
            ));
        }
        if !seen.insert(&gate.id) {
            return Err(VerifyError::InvalidGate(format!(
                "duplicate gate id '{}'",
                gate.id
            )));
        }
    }
    Ok(())
}
