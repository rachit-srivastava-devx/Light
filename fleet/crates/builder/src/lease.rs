use serde::{Deserialize, Serialize};

use crate::{launch::BuilderError, Observation};

/// Immutable lease that bounds a single worker invocation.
///
/// `generation`, `base`, and `write_scope` are set by the parent at grant
/// time and must never be mutated by the worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lease {
    /// Absolute path to the private worktree base.
    pub base: String,
    /// Content digest of the accepted plan this worker executes.
    pub plan_digest: String,
    /// Path prefixes the worker is permitted to write (e.g. `["src/"]`).
    pub write_scope: Vec<String>,
    /// Monotonic generation counter; stale duplicates are rejected.
    pub generation: u64,
}

/// Return `Err(ScopeViolation)` for any path not covered by a scope prefix.
fn validate_scope(paths: &[String], write_scope: &[String]) -> Result<(), BuilderError> {
    for path in paths {
        let in_scope = write_scope.iter().any(|s| path.starts_with(s.as_str()));
        if !in_scope {
            return Err(BuilderError::ScopeViolation(format!(
                "{path:?} is outside the declared write_scope"
            )));
        }
    }
    Ok(())
}

/// Validate `o` against `lease` scope constraints.
///
/// Returns `Err(ScopeViolation)` when:
/// - `changed` is empty (no-op worker stub)
/// - any entry in `changed` falls outside `write_scope`
pub(crate) fn validate_observation(o: &Observation, lease: &Lease) -> Result<(), BuilderError> {
    if o.changed.is_empty() {
        return Err(BuilderError::ScopeViolation(
            "empty write set is not a valid candidate".to_string(),
        ));
    }
    validate_scope(&o.changed, &lease.write_scope)
}
