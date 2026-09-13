use crate::explain::CandidateId;

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
