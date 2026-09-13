//! Integration tests for the `builder` crate.
//!
//! Named tests in §10 of the blueprint live here. Each test exercises one
//! mutation target from §11 — removing the guard causes the assertion to fail.

use builder::{parse_fd3_frame, validate_observation, BuilderError, Lease, Observation};

fn lease(scope: &[&str]) -> Lease {
    Lease {
        base: "worktree".to_string(),
        plan_digest: "deadbeef".to_string(),
        write_scope: scope.iter().map(|s| s.to_string()).collect(),
        generation: 1,
    }
}

fn obs(changed: &[&str]) -> Observation {
    Observation {
        tree_digest: "cafebabe".to_string(),
        changed: changed.iter().map(|s| s.to_string()).collect(),
        unexpected: vec![],
        exit_code: 0,
        fd3_digest: "feed1234".to_string(),
    }
}

mod tests {
    use super::*;

    /// Mutation target: removing the out-of-scope path check in `validate_scope`.
    /// A path outside `write_scope` must produce `ScopeViolation`.
    #[test]
    fn scope_violation_refused() {
        let l = lease(&["src/"]);
        let o = obs(&["tests/acceptance.rs"]);
        let result = validate_observation(&o, &l);
        assert!(
            matches!(result, Err(BuilderError::ScopeViolation(_))),
            "expected ScopeViolation for out-of-scope path; got {result:?}",
        );
    }

    /// Mutation target: accepting an empty fd-3 frame as a valid result.
    /// Exit-0 alone is not proof of success — a well-formed fd-3 frame is required.
    #[test]
    fn fd3_frame_required_for_success() {
        let none_result = parse_fd3_frame(None);
        assert!(
            matches!(none_result, Err(BuilderError::Protocol(_))),
            "absent fd-3 must be Protocol error; got {none_result:?}",
        );
        let empty_result = parse_fd3_frame(Some(b""));
        assert!(
            matches!(empty_result, Err(BuilderError::Protocol(_))),
            "empty fd-3 must be Protocol error; got {empty_result:?}",
        );
    }

    /// Mutation target: allowing an empty `changed` set to pass as a valid candidate.
    /// A no-op observation must be rejected before it reaches review.
    #[test]
    fn empty_write_set_refused() {
        let l = lease(&["src/"]);
        let o = obs(&[]);
        let result = validate_observation(&o, &l);
        assert!(
            matches!(result, Err(BuilderError::ScopeViolation(_))),
            "expected ScopeViolation for empty write set; got {result:?}",
        );
    }
}
