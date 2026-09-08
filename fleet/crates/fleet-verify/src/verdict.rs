//! Verdict and per-gate result -- the outcome of orchestrating one `GateSpec`.

use crate::denominator::Denominator;

/// Why a gate's stdout could not be trusted as a `Pass` even though the wrapped process exited 0.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailReason {
    /// The wrapped process exited nonzero; the raw code is kept for diagnostics, not normalized.
    NonZeroExit(i32),
    /// Exit was 0, but `parse_denominator` returned `Counted(_, 0)` -- the gate measured nothing.
    MeasuredNothing,
    /// Exit was 0, but `parse_denominator` returned `Unparseable` -- no recognizable denominator
    /// marker at all.
    Unparseable,
}

/// One gate's outcome after orchestration. Never constructed by a `GateSpec`'s own
/// `parse_denominator` -- only `classify` builds one.
#[derive(Clone, Debug)]
pub enum Verdict {
    Pass(Denominator),
    Fail {
        reason: FailReason,
        denominator: Option<Denominator>,
    },
    Skip {
        reason: String,
        was_required: bool,
    },
}

#[derive(Clone, Debug)]
pub struct GateResult {
    pub id: &'static str,
    pub verdict: Verdict,
}
