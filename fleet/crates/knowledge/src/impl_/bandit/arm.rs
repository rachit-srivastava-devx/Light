//! `ArmStats` and `NoArms`. See BLUEPRINT.md §3.G.

/// One bandit arm's observed history. `id` matches a `fleet-router` `CandidateSpec::id` string
/// by convention; this crate does not import `fleet-router` (no sibling-to-sibling DAG edge).
#[derive(Clone, Copy, Debug)]
pub struct ArmStats {
    pub id: &'static str,
    pub successes: u64,
    pub failures: u64,
}

/// `pick_arm` was called with an empty `arms` slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("pick_arm requires at least one arm")]
pub struct NoArms;
