//! fleet-plan: the spec-production pipeline's decisions -- intake, LLD draft, the LLD-ready
//! gate, review verdicts, and teach-back -- as pure, typed functions. Zero IO, zero clock, zero
//! RNG, zero subprocess of its own; every fact this crate needs is a parameter, not a read.

pub mod intake;
pub mod lld_draft;
pub mod ready_gate;
pub mod review;
pub mod teach;

pub use intake::{
    derive_questions, intake_gate, open_questions, validate_atomic_rows, validate_challenge_rows,
    validate_clarification_rows, validate_sow_text, AtomicRow, AtomicTier, ChallengeRow,
    ClarificationKind, ClarificationRow, GateDecision, IntakeQuestion, QuestionId, StageReadiness,
    StageViolation, Trigger, UnknownQuestionId,
};
pub use lld_draft::{assemble_acceptance_checks_draft, assemble_blueprint_doc, assemble_summary_doc};
pub use ready_gate::{
    canonical_json, check_and_evaluate, content_hash, depth_evidence, evaluate, evaluate_with,
    validate_lld_v1, validate_module_brief, Check, DepthScore, EntryOutcome, GateReason, GateRefs,
    NumericLeafError, Outcome, Verdict, Violation, CHECKS, GATE_CHECK_IDS, STAMPED_BY,
};
pub use review::{
    submission_eligible, validate_review_contract, verdict_decision, ReviewContractViolation,
    RoleContract, SubmissionRefusal, VerdictDecision, VerdictName, VerdictRefusal,
};
pub use teach::{derive_lesson, Lesson, LessonSource, TaughtOutcome};
