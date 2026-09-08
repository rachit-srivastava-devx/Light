//! `build_pr_walkthrough` -- the deterministic control-plane assembly for ask #19 ("teach the PR
//! it created to the user in detail"). Pure: diff summary, module brief, acceptance results, and
//! attestation are all caller-supplied; no IO/clock/RNG/model call.

use super::error::PrWalkthroughError;
use super::risk::{reviewer_focus, riskiest_part};
use super::types::{AcceptanceResult, AttestationSummary, DiffSummary, PrWalkthrough, VerifiedItem};
use crate::ready_gate::validate_module_brief;
use crate::walkthrough::extract::extract_module_summary;
use serde_json::Value;

pub fn build_pr_walkthrough(
    module_brief: &Value,
    diff: &DiffSummary,
    acceptance_results: &[AcceptanceResult],
    attestation: &AttestationSummary,
) -> Result<PrWalkthrough, PrWalkthroughError> {
    if diff.files.is_empty() {
        return Err(PrWalkthroughError::EmptyDiff);
    }
    if acceptance_results.is_empty() {
        return Err(PrWalkthroughError::NoAcceptanceResults);
    }
    let violations = validate_module_brief(module_brief);
    if !violations.is_empty() {
        return Err(PrWalkthroughError::InvalidModuleBrief { violations });
    }
    let summary = extract_module_summary(module_brief).ok_or(PrWalkthroughError::IncompleteModuleBrief)?;

    let (added, removed) = diff.files.iter().fold((0u32, 0u32), |(a, r), f| (a + f.lines_added, r + f.lines_removed));
    let what_changed = format!(
        "{}: {} file(s) changed, +{added}/-{removed} lines",
        summary.node_id,
        diff.files.len()
    );
    let why = summary.purpose.clone();
    let riskiest = riskiest_part(diff, acceptance_results);
    let focus = reviewer_focus(attestation, acceptance_results, &riskiest);

    let verified = acceptance_results
        .iter()
        .filter(|a| a.passed)
        .map(|a| VerifiedItem { check_name: a.check_name.clone(), detail: a.detail.clone() })
        .collect();
    let not_verified = acceptance_results
        .iter()
        .filter(|a| !a.passed)
        .map(|a| VerifiedItem { check_name: a.check_name.clone(), detail: a.detail.clone() })
        .collect();

    Ok(PrWalkthrough { what_changed, why, riskiest_part: riskiest, verified, not_verified, reviewer_focus: focus })
}
