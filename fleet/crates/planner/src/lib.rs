//! Module planner — plan a sequence of modules given a DAG.
//! Re-exports plan for backward compat while new implementation matures.
//!
//! STATUS: `validate_draft`/`PlannerModel`/`PlanInput` (this crate's own family, distinct from
//! `propose`'s now-deleted dead duplicate) are unwired scaffolding matching `docs/LLD/LLD.md` —
//! not dead code, no caller yet.
pub use plan::*;

mod propose;
mod types;
mod validate;

pub use propose::PlannerModel;
pub use types::{ModuleDraft, PlanDraft, PlanInput, PlannerError, ValidationReport};
pub use validate::validate_draft;
