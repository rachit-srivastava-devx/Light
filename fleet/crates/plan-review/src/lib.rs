//! `plan-review` — independent review gate for plan proposals.
//! Blueprint: docs/blueprints-next/plan_review/BLUEPRINT.md
//! C9 enforced: all model calls route through the injected PlanReviewer port.

pub mod contract;
pub mod types;
pub mod verdict;

pub use contract::{validate, validate_plan_proposal};
pub use types::{
    Decision, Finding, PlanProposal, PlanReviewer, PlanWalkthrough, ReviewError, ReviewEvent,
    ReviewInput, ReviewVerdict, ReviewedPlanDigest,
};
pub use verdict::{call_independent_model, emit_walkthrough, verdict_to_digest};

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
