//! Module planner — plan a sequence of modules given a DAG.
//! Re-exports fleet-plan for backward compat while new implementation matures.
pub use fleet_plan::*;

mod propose;
mod types;
mod validate;

pub use propose::PlannerModel;
pub use types::{ModuleDraft, PlanDraft, PlanInput, PlannerError, ValidationReport};
pub use validate::validate_draft;
