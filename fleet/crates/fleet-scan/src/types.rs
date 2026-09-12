//! Versioned records for the LLD materiality gate.

use thiserror::Error;

/// A single unresolved decision point in a proposed plan.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Unknown {
    pub field: String,
    pub alternatives: Vec<String>,
    pub effect_changes: bool,
    pub acceptance_changes: bool,
    pub missing_grant: bool,
    pub blocks_ready_node: bool,
}

/// Input to the materiality scan: a set of unknowns and the plan revision they belong to.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ScanInput {
    pub unknowns: Vec<Unknown>,
    pub revision: u64,
}

/// The scan verdict: either nothing material was found, or the bounded probe set to run.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ScanDecision {
    Clear { revision: u64 },
    Probe { kinds: Vec<crate::ProbeKind>, revision: u64 },
}

/// Typed errors from the materiality scan.
#[derive(Clone, Debug, PartialEq, Error)]
pub enum ScanError {
    #[error("revision must be nonzero")]
    EmptyRevision,
    #[error("duplicate field in unknowns: {0}")]
    DuplicateField(String),
    #[error("too many unknowns (maximum 256)")]
    TooManyUnknowns,
}
