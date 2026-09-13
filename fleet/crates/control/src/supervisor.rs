use crate::{AuthorityStore, ControlError};

/// Spawn decision returned after a successful commit.
pub enum SpawnDecision {
    Launch { task_id: String },
    Skip,
}

/// Commit-before-spawn: writes state to the store FIRST, then returns the
/// spawn decision. Ensures durable authority exists before any child is
/// launched — spawn-before-commit is a correctness violation caught by the
/// recovery test.
pub fn commit_then_decide(
    store: &dyn AuthorityStore,
    task_id: &str,
    should_spawn: bool,
) -> Result<SpawnDecision, ControlError> {
    store.write_state(task_id)?;
    if should_spawn {
        Ok(SpawnDecision::Launch {
            task_id: task_id.to_string(),
        })
    } else {
        Ok(SpawnDecision::Skip)
    }
}

/// Validate that a child's generation matches the lease generation.
/// Stale children (old generation) cannot commit effects — reject them.
pub fn validate_generation(child_gen: u64, lease_gen: u64) -> Result<(), ControlError> {
    if child_gen != lease_gen {
        Err(ControlError::Store(format!(
            "stale child generation: child={child_gen} lease={lease_gen}"
        )))
    } else {
        Ok(())
    }
}
