//! Intake -- fleet/registry-reference/registry/features/intake/intake.sh

mod atomic;
mod atomic_types;
mod challenges;
mod clarifications;
mod gate;
mod pattern;
mod pattern_custom;
mod question_id;
mod rubric;
mod rubric_defs;
mod sow;
mod sow_checks;

pub use atomic::validate_atomic_rows;
pub use atomic_types::{AtomicRow, AtomicTier};
pub use challenges::{validate_challenge_rows, ChallengeRow};
pub use clarifications::{validate_clarification_rows, ClarificationKind, ClarificationRow};
pub use gate::{intake_gate, GateDecision, StageReadiness};
pub use question_id::{QuestionId, UnknownQuestionId};
pub use rubric::{derive_questions, open_questions, IntakeQuestion, Trigger};
pub use sow::{validate_sow_text, StageViolation};
