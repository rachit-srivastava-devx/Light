//! Module planner — plan a sequence of modules given a DAG.
//! Re-exports plan for backward compat while new implementation matures.
pub use plan::*;

mod propose;
mod types;
mod validate;

pub use propose::PlannerModel;
pub use types::{ModuleDraft, PlanDraft, PlanInput, PlannerError, ValidationReport};
pub use validate::validate_draft;
