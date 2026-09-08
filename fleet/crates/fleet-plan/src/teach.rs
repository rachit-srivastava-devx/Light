//! Turning one settled outcome into a `Lesson` -- greenfield, no such assembly exists in fleet
//! today; `intake.sh`'s `challenge_source_exists` only ever reads existing `challenges.tsv` rows.

use fleet_types::{NodeId, Role, TaskId};

/// Mirrors `challenge_source_exists`'s two recognised prefixes (`intake.sh:236-242`) as a closed
/// type instead of a colon-split string.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LessonSource {
    FailureCorpus(String),
    ThreadLessons(String),
}

impl LessonSource {
    /// The exact `"FAILURE-CORPUS:<key>"` / `"THREAD-LESSONS:<key>"` wire form
    /// `challenge_source_exists` pattern-matches on (`intake.sh:237,238`).
    pub fn as_wire_ref(&self) -> String {
        match self {
            LessonSource::FailureCorpus(key) => format!("FAILURE-CORPUS:{key}"),
            LessonSource::ThreadLessons(key) => format!("THREAD-LESSONS:{key}"),
        }
    }
}

/// The one thing that happened and is now being turned into a lesson.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TaughtOutcome {
    Rejected { reviewer_role: String, reason: String },
    Escalated { task: TaskId, attempts: u32, limit: u32 },
    GateRefused { check_id: &'static str, detail: String },
    MutationSurvived { mutant: String, killed_by: Option<String> },
}

/// The `challenges.tsv` row shape (`id\tsource\taffected_leaf\trisk\ttrigger\tmitigation`,
/// `intake.sh:246`) reused verbatim as the wire shape a teach step emits.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lesson {
    pub source: LessonSource,
    pub affected_leaf: NodeId,
    pub risk: String,
    pub trigger: String,
    pub mitigation: String,
}

/// Turns one settled outcome into a `Lesson` ready for the caller to append.
pub fn derive_lesson(outcome: &TaughtOutcome, source: LessonSource, affected_leaf: NodeId, role: Role) -> Lesson {
    let (risk, trigger, mitigation) = match outcome {
        TaughtOutcome::Rejected { reviewer_role, reason } => (
            format!("a {} role's submission was rejected by {reviewer_role}", role.name()),
            format!("reviewer rejected with: {reason}"),
            "address the reviewer's stated reason before resubmitting".to_string(),
        ),
        TaughtOutcome::Escalated { task, attempts, limit } => (
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
    Lesson { source, affected_leaf, risk, trigger, mitigation }
}
