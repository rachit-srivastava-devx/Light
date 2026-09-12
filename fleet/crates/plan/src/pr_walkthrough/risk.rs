//! Picks the one riskiest part of a PR and ranks what a reviewer should check first --
//! deterministic priority order, never a coin flip between equally-plausible candidates.

use super::types::{AcceptanceResult, AttestationSummary, DiffSummary};
use crate::review::VerdictName;

/// Priority: a failed acceptance check outranks everything (it's a known break); absent that,
/// the single largest-churn file (most lines touched) is the most-changed, least-obvious spot.
pub(crate) fn riskiest_part(diff: &DiffSummary, acceptance: &[AcceptanceResult]) -> String {
    if let Some(failed) = acceptance.iter().find(|a| !a.passed) {
        return format!("acceptance check {:?} failed: {}", failed.check_name, failed.detail);
    }
    let biggest = diff
        .files
        .iter()
        .max_by_key(|f| (f.lines_added + f.lines_removed, std::cmp::Reverse(f.path.clone())));
    match biggest {
        Some(f) => format!("{} has the largest diff ({} lines touched) -- review it first", f.path, f.lines_added + f.lines_removed),
        None => "no changed files carry a line-count signal; review the diff as a whole".to_string(),
    }
}

/// Ranked, ADHD-shaped: attestation disagreement first (it's a stop sign), then every failed
/// check by name, then the riskiest file, deduplicated but order-preserving.
pub(crate) fn reviewer_focus(attestation: &AttestationSummary, acceptance: &[AcceptanceResult], riskiest: &str) -> Vec<String> {
    let mut out = Vec::new();
    if !matches!(attestation.verdict, VerdictName::Accept) {
        out.push(format!("attestation verdict is {:?}, not Accept -- read {}'s notes first", attestation.verdict, attestation.builder));
    }
    for a in acceptance.iter().filter(|a| !a.passed) {
        out.push(format!("{} did not pass: {}", a.check_name, a.detail));
    }
    out.push(riskiest.to_string());
    out
}
