//! Typed refusals for `build_walkthrough` -- greenfield: original ask #6 ("teach the user on
//! the implementation suggested, walk him through it") has no prior code in fleet to port from.

use crate::ready_gate::Violation;

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum WalkthroughError {
    /// A plan with zero modules (or the caller passing an empty slice for a blank draft) cannot
    /// be walked through -- refuse instead of silently returning an empty `Walkthrough`.
    #[error("cannot build a walkthrough for an empty or blank plan (0 modules)")]
    EmptyPlan,
    /// The module brief at `index` fails `ready_gate::validate_module_brief` -- a walkthrough
    /// must never narrate a module brief this crate would otherwise reject.
    #[error("module brief at index {index} failed shape validation ({} violation(s))", violations.len())]
    InvalidModuleBrief { index: usize, violations: Vec<Violation> },
    /// The brief passed shape validation but a field this walkthrough needs (node_id/purpose/
    /// owner) was still unreadable -- defensive, should be unreachable given the check above.
    #[error("module brief at index {index} is missing a required field after validation")]
    IncompleteModuleBrief { index: usize },
    /// Two module briefs in the same plan declare the same `node_id`.
    #[error("duplicate node_id {0:?} across module briefs")]
    DuplicateNodeId(String),
    /// The `deps` graph among the plan's own modules is not a DAG -- no work order exists.
    #[error("dependency cycle detected among modules: {cycle:?}")]
    CyclicDependency { cycle: Vec<String> },
}
