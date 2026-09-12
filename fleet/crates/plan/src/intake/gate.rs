//! Byte-faithful port of `gate()`'s decision tree (`intake.sh:311-322`), stripped of the
//! `rubric_gate`/`stage_ready`/`open_blockers` file reads it interleaves today -- each is now a
//! caller-supplied fact.

/// The 4-stage readiness a caller has already independently checked.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StageReadiness {
    pub sow: bool,
    pub atomic: bool,
    pub challenges: bool,
    pub clarifications: bool,
}

impl StageReadiness {
    pub fn all_ready(self) -> bool {
        self.sow && self.atomic && self.challenges && self.clarifications
    }
}

/// Why code-start is refused, matching `gate()`'s three named reasons (`intake.sh:311-322`) plus
/// the success case.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GateDecision {
    Ready,
    BlockedByRubric,
    BlockedByStages,
    BlockedByOpenClarifications { count: u32 },
}

/// Byte-faithful port of `gate()`. Priority order matches the bash `if` chain exactly: rubric
/// first, then stages, then open blocking clarifications.
pub fn intake_gate(
    rubric_open: usize,
    stages: StageReadiness,
    open_blocking_clarifications: u32,
) -> GateDecision {
    if rubric_open != 0 {
        return GateDecision::BlockedByRubric;
    }
    if !stages.all_ready() {
        return GateDecision::BlockedByStages;
    }
    if open_blocking_clarifications != 0 {
        return GateDecision::BlockedByOpenClarifications { count: open_blocking_clarifications };
    }
    GateDecision::Ready
}
