//! Typed refusals for `build_pr_walkthrough` -- greenfield: ask #19 ("teach the PR it created to
//! the user in detail") has no prior code in fleet to port from.

use crate::ready_gate::Violation;

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum PrWalkthroughError {
    /// A diff with zero changed files describes no PR -- refuse rather than narrate nothing.
    #[error("cannot build a PR walkthrough for a diff with no changed files")]
    EmptyDiff,
    /// The module brief this PR implements fails `ready_gate::validate_module_brief`.
    #[error("module brief failed shape validation ({} violation(s))", violations.len())]
    InvalidModuleBrief { violations: Vec<Violation> },
    /// The brief passed shape validation but a required field was still unreadable -- defensive.
    #[error("module brief is missing a required field after validation")]
    IncompleteModuleBrief,
    /// No acceptance results were run for this change -- "what was verified" would be a lie.
    #[error("cannot build a PR walkthrough with zero acceptance results")]
    NoAcceptanceResults,
}
