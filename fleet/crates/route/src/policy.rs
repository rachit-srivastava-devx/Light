#[derive(Clone, Debug)]
pub struct PolicySnapshot {
    pub max_cost_per_call: u64,
    pub allowed_providers: Vec<String>,
}
