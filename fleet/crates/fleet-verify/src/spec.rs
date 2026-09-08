//! One committed gate's identity and policy -- mirrors one `stage "name" ...` line in
//! `verify.sh`.

use crate::denominator::DenominatorResult;
use crate::requirement::{ProbeTool, Requirement};

/// One gate's identity and policy, as committed data.
#[derive(Clone, Copy)]
pub struct GateSpec {
    /// Stable identity used in `GateResult`/`Report`. Never renamed once shipped.
    pub id: &'static str,
    pub requirement: Requirement,
    pub probe: ProbeTool,
    pub command: &'static [&'static str],
    /// Extracts this gate's published denominator from its own stdout/stderr. A fn pointer (not
    /// a closure) so `GateSpec` stays `Copy`.
    pub parse_denominator: fn(stdout: &str, stderr: &str) -> DenominatorResult,
}
