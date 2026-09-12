//! The `Probe` trait and the raw candidate-question shape it produces.

use crate::fault::EnvFault;
use crate::input::{ProbeKind, RequirementInput};

/// One ambiguity-probing strategy. Every impl is read-only: it must not write to disk, mutate any
/// external state, or block waiting on anything not reachable through its own injected port.
pub trait Probe: Send + Sync {
    fn kind(&self) -> ProbeKind;
    /// Examine `input` and return zero or more candidate questions, or a fault if this probe's
    /// port could not be reached. Never panics by contract (a bug that panics is caught by
    /// `ConcurrentRunner`, not by this fn) and never blocks beyond its port's own call.
    fn probe(&self, input: &RequirementInput) -> ProbeOutcome;
}

/// What one `Probe::probe` call produced.
#[derive(Clone, Debug, PartialEq)]
pub enum ProbeOutcome {
    Questions(Vec<Question>),
    Fault(EnvFault),
}

/// One candidate ambiguity question, before merge/dedup/rank/cap.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Question {
    pub probe: ProbeKind,
    pub text: String,
    /// Why this ambiguity matters -- mandatory content, not mandatory non-emptiness at
    /// construction (a probe may legitimately emit a malformed row; `merge_questions` is the
    /// enforcement boundary, see §6). A `Question` with an empty `why` is representable on
    /// purpose so the merge step has something concrete to drop and a test can assert it drops it.
    pub why: String,
    pub gap: GapSeverity,
    /// Optional grounding -- a file:line, a memory-hit id, a prior decision reference -- so a
    /// human reading the question can see where it came from. Never required.
    pub evidence: Option<String>,
}

/// How severe the ambiguity gap is. Ordered so `Blocking` sorts first (§6 "sort by gap").
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GapSeverity {
    Low,
    Medium,
    High,
    Blocking,
}
