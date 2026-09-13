use crate::{explain::StageEvidence, policy::PolicySnapshot};

#[path = "filter_types.rs"]
mod filter_types;
pub use filter_types::{BudgetSnapshot, Candidate, CatalogSnapshot, IntentSpec};

pub fn capability_filter(
    intent: &IntentSpec,
    snapshot: &CatalogSnapshot,
) -> (Vec<Candidate>, StageEvidence) {
    let n_in = snapshot.candidates.len();
    let survivors: Vec<Candidate> = snapshot
        .candidates
        .iter()
        .filter(|c| {
            intent
                .required_capabilities
                .iter()
                .all(|req| c.capabilities.contains(req))
        })
        .cloned()
        .collect();
    let n_out = survivors.len();
    (
        survivors,
        StageEvidence {
            stage: "capability".into(),
            candidates_in: n_in,
            candidates_out: n_out,
        },
    )
}

pub fn policy_filter(
    candidates: Vec<Candidate>,
    policy: &PolicySnapshot,
) -> (Vec<Candidate>, StageEvidence) {
    let n_in = candidates.len();
    let survivors: Vec<Candidate> = candidates
        .into_iter()
        .filter(|c| {
            let prov_ok = policy.allowed_providers.is_empty()
                || policy.allowed_providers.contains(&c.provider);
            let cost_ok = c
                .historical_cost
                .is_none_or(|v| v <= policy.max_cost_per_call);
            prov_ok && cost_ok
        })
        .collect();
    let n_out = survivors.len();
    (
        survivors,
        StageEvidence {
            stage: "policy".into(),
            candidates_in: n_in,
            candidates_out: n_out,
        },
    )
}

pub fn budget_filter(
    candidates: Vec<Candidate>,
    budget: &BudgetSnapshot,
) -> (Vec<Candidate>, StageEvidence) {
    let n_in = candidates.len();
    let available = budget.limit.saturating_sub(budget.spent);
    let survivors: Vec<Candidate> = candidates
        .into_iter()
        .filter(|c| c.historical_cost.is_none_or(|v| v <= available))
        .collect();
    let n_out = survivors.len();
    (
        survivors,
        StageEvidence {
            stage: "budget".into(),
            candidates_in: n_in,
            candidates_out: n_out,
        },
    )
}
