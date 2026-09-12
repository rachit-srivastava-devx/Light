use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::{
    CandidateObservation, ContextManifest, Decision, Finding, FindingsProvider,
    ReviewError, ReviewResult, ReviewedCandidate,
};
use crate::findings::apply_scope_filter;

fn quick_hash(s: &str) -> String {
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    format!("{:016x}", h.finish())
}

fn populate_output_digest(findings: &[Finding]) -> String {
    let s: String = findings
        .iter()
        .map(|f| format!("{}:{}", f.path, f.severity))
        .collect::<Vec<_>>()
        .join("|");
    quick_hash(if s.is_empty() { "empty" } else { &s })
}

pub fn check_reviewer_independence(
    worker_id: &str,
    reviewer_id: &str,
) -> Result<(), ReviewError> {
    if worker_id == reviewer_id {
        return Err(ReviewError::NotIndependent);
    }
    Ok(())
}

/// Assemble a `ReviewedCandidate` from an observation, optional scope manifest,
/// and an injected findings provider (mock-friendly for tests).
pub fn assemble_reviewed_candidate(
    obs: &CandidateObservation,
    manifest: Option<&ContextManifest>,
    provider: Box<dyn FindingsProvider>,
) -> Result<ReviewedCandidate, ReviewError> {
    if obs.diff.is_empty() {
        return Err(ReviewError::EmptyObservation);
    }
    check_reviewer_independence(&obs.worker_id, &obs.reviewer_id)?;

    let raw = provider.findings(&obs.diff);
    let findings = match manifest {
        Some(m) => apply_scope_filter(raw, &m.scope),
        None => raw,
    };

    let total = obs.changed_paths.len() as u64;
    let checked = total.max(1);
    let input_digest = quick_hash(&format!(
        "{}|{}|{}",
        obs.tree_digest, obs.plan_digest, obs.acceptance_digest
    ));
    let output_digest = populate_output_digest(&findings);
    let passed = !findings
        .iter()
        .any(|f| f.severity == "WARNING" || f.severity == "ERROR");
    let decision = if passed { Decision::Approved } else { Decision::ChangesRequested };

    Ok(ReviewedCandidate {
        passed,
        result: ReviewResult { decision, findings, checked, total, input_digest, output_digest },
    })
}
