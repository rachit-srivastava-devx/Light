//! Shared test helpers (not a test binary itself -- `tests/common/` is excluded from discovery).
#![allow(dead_code)]

use fleet_scan::{ConcurrentRunner, GapSeverity, Probe, ProbeJob, ProbeKind, ProbeOutcome, ProbeRun, Question, RequirementInput};

pub fn input(text: &str) -> RequirementInput {
    RequirementInput { text: text.into(), task_id: None }
}

pub fn q(text: &str, why: &str, gap: GapSeverity, probe: ProbeKind) -> Question {
    Question { probe, text: text.into(), why: why.into(), gap, evidence: None }
}

/// `Question` with a fixed non-empty `why`, for tests that only care about text/gap/probe.
pub fn qw(text: &str, gap: GapSeverity, probe: ProbeKind) -> Question {
    q(text, "why", gap, probe)
}

/// A probe test-double that always returns the same `ProbeOutcome`.
pub struct FixedProbe {
    pub kind: ProbeKind,
    pub outcome: ProbeOutcome,
}
impl Probe for FixedProbe {
    fn kind(&self) -> ProbeKind {
        self.kind
    }
    fn probe(&self, _input: &RequirementInput) -> ProbeOutcome {
        self.outcome.clone()
    }
}

/// Runs jobs sequentially in-process -- deterministic completion order for tests.
pub struct SequentialRunner;
impl ConcurrentRunner for SequentialRunner {
    fn run4<'a>(&self, jobs: [ProbeJob<'a>; 4]) -> [ProbeRun; 4] {
        jobs.map(|job| ProbeRun::Completed(job()))
    }
}
