use crate::contract::validate_plan_proposal;
use crate::types::{
    Decision, PlanProposal, PlanReviewer, PlanWalkthrough, ReviewError, ReviewEvent, ReviewInput,
    ReviewVerdict, ReviewedPlanDigest,
};

/// Dispatch to the injected reviewer model and return an event sequence.
/// Emits walkthrough BEFORE the digest — structural ordering contract.
pub fn call_independent_model(
    proposal: &PlanProposal,
    reviewer_id: &str,
    reviewer: &dyn PlanReviewer,
) -> Result<Vec<ReviewEvent>, ReviewError> {
    validate_plan_proposal(proposal, reviewer_id)?;
    let input = ReviewInput {
        plan_digest: proposal.plan_digest.clone(),
        reviewer_id: reviewer_id.to_string(),
        worker_id: proposal.worker_id.clone(),
        evidence_digest: proposal.evidence_digest.clone(),
    };
    let verdict = reviewer.review(&input)?;
    let events = emit_walkthrough(proposal, reviewer_id, &verdict);
    Ok(events)
}

/// Build the ordered event sequence: walkthrough first, then digest.
pub fn emit_walkthrough(
    proposal: &PlanProposal,
    reviewer_id: &str,
    verdict: &ReviewVerdict,
) -> Vec<ReviewEvent> {
    let walkthrough = PlanWalkthrough {
        plan_digest: proposal.plan_digest.clone(),
        reviewer_id: reviewer_id.to_string(),
        findings: verdict.findings.clone(),
    };
    let approved = verdict.decision == Decision::Accept && verdict.checked > 0;
    let digest = ReviewedPlanDigest {
        plan_digest: proposal.plan_digest.clone(),
        reviewer_id: reviewer_id.to_string(),
        approved,
        checked: verdict.checked,
        total: verdict.total,
    };
    vec![
        ReviewEvent::Walkthrough(walkthrough),
        ReviewEvent::Digest(digest),
    ]
}

/// Canonical output digest from the verdict.
pub fn verdict_to_digest(verdict: &ReviewVerdict) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    verdict.output_digest.hash(&mut h);
    verdict.checked.hash(&mut h);
    format!("{:016x}", h.finish())
}
