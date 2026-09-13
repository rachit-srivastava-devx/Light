//! Turning one settled outcome into a `Lesson` -- greenfield, no such assembly exists in fleet
//! today; `intake.sh`'s `challenge_source_exists` only ever reads existing `challenges.tsv` rows.

use types::{NodeId, Role};

#[path = "lesson_types.rs"]
mod lesson_types;
pub use lesson_types::{Lesson, LessonSource, TaughtOutcome};

/// Turns one settled outcome into a `Lesson` ready for the caller to append.
pub fn derive_lesson(
    outcome: &TaughtOutcome,
    source: LessonSource,
    affected_leaf: NodeId,
    role: Role,
) -> Lesson {
    let (risk, trigger, mitigation) = match outcome {
        TaughtOutcome::Rejected {
            reviewer_role,
            reason,
        } => (
            format!(
                "a {} role's submission was rejected by {reviewer_role}",
                role.name()
            ),
            format!("reviewer rejected with: {reason}"),
            "address the reviewer's stated reason before resubmitting".to_string(),
        ),
        TaughtOutcome::Escalated {
            task,
            attempts,
            limit,
        } => (
            format!("task {} exhausted its review retry budget", task.as_str()),
            format!("attempt {attempts} reached the {limit}-attempt ceiling"),
            "escalate to a higher-authority reviewer instead of retrying".to_string(),
        ),
        TaughtOutcome::GateRefused { check_id, detail } => (
            format!("the {check_id} depth-readiness check failed"),
            detail.clone(),
            format!("satisfy {check_id} before the gate will report Ready"),
        ),
        TaughtOutcome::MutationSurvived { mutant, killed_by } => (
            "a mutant survived the mutation-testing suite".to_string(),
            format!("mutant {mutant} was not killed by any test"),
            match killed_by {
                Some(test) => format!("strengthen {test} to kill this mutant"),
                None => "no test currently kills this mutant; write one".to_string(),
            },
        ),
    };
    Lesson {
        source,
        affected_leaf,
        risk,
        trigger,
        mitigation,
    }
}
