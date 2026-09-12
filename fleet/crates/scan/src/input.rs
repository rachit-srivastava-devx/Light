//! Requirement input and the fixed 4-probe identity enum.

use fleet_types::TaskId;

/// The requirement text to probe, plus optional correlation metadata. This crate never mutates
/// or persists it -- `text` is read, nothing else.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequirementInput {
    /// The raw requirement/task description as written by the human or upstream stage.
    pub text: String,
    /// Correlates this assessment with a task for receipt-writing by the caller. This crate never
    /// reads or writes anything keyed on it -- it is opaque pass-through metadata.
    pub task_id: Option<TaskId>,
}

/// Which of the 4 fixed probes produced a `Question` or `EnvFault`. Fixed set, not extensible at
/// runtime -- adding a 5th probe is a change to this enum and to `ProbeSet`, reviewed like any
/// other API change, not a runtime plugin registration.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProbeKind {
    Business,
    Technical,
    Memory,
    Research,
}
