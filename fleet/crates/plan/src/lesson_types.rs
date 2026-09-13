use types::{NodeId, TaskId};

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
    Rejected {
        reviewer_role: String,
        reason: String,
    },
    Escalated {
        task: TaskId,
        attempts: u32,
        limit: u32,
    },
    GateRefused {
        check_id: &'static str,
        detail: String,
    },
    MutationSurvived {
        mutant: String,
        killed_by: Option<String>,
    },
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
