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
    pub command: GateCommand,
    /// Extracts this gate's published denominator from its own stdout/stderr. A fn pointer (not
    /// a closure) so `GateSpec` stays `Copy`.
    pub parse_denominator: fn(stdout: &str, stderr: &str) -> DenominatorResult,
}

/// How a gate's argv is built. Split from a single `&'static [&'static str]` (the old shape) so a
/// script gate's path can be resolved against a `GatesRoot` at run time instead of being a
/// `bin/...`-relative literal that only works when the process's cwd is a fleet checkout.
#[derive(Clone, Copy)]
pub enum GateCommand {
    /// Run directly via `$PATH` -- no gates-root resolution needed (e.g. `cargo test`).
    OnPath(&'static [&'static str]),
    /// Run a gate script resolved against a `GatesRoot`, plus any extra argv after it.
    Script {
        relative: &'static str,
        args: &'static [&'static str],
    },
}
