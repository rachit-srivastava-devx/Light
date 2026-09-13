use crate::explain::{CandidateId, StageEvidence};
use crate::policy::PolicySnapshot;

#[derive(Clone, Debug)]
pub struct Candidate {
    pub id: CandidateId,
    pub capabilities: Vec<String>,
    pub historical_cost: Option<u64>,
    pub provider: String,
}

#[derive(Clone, Debug)]
pub struct IntentSpec {
    pub required_capabilities: Vec<String>,
    pub task_class: String,
}

#[derive(Clone, Debug)]
pub struct CatalogSnapshot {
    pub candidates: Vec<Candidate>,
    pub digest: String,
}

#[derive(Clone, Debug)]
pub struct BudgetSnapshot {
    pub limit: u64,
    pub spent: u64,
}

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
