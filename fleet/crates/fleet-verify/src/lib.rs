//! Typed orchestration of fleet's shell verification gates.
//!
//! This crate answers one question -- "given the committed set of verification gates, what did
//! each one actually prove" -- without spawning a subprocess or probing a tool itself: both are
//! injected via `ToolProbe`/`ProcessRunner`. The one non-negotiable rule enforced here that no
//! individual gate script is trusted to enforce on its own: a gate that reports it measured zero
//! of anything is a `Fail`, never a silent `Pass`, no matter what exit code its script returned.

mod classify;
mod denominator;
mod digits;
mod orchestrate;
mod parsers;
mod ports;
mod registry;
mod report;
mod requirement;
mod spec;
mod verdict;

pub use denominator::{Denominator, DenominatorResult, ZeroDenominator};
pub use orchestrate::{run_all, run_gate};
pub use ports::{ProcessOutput, ProcessRunner, ToolProbe};
pub use registry::GATES;
pub use report::Report;
pub use requirement::{ProbeTool, Requirement};
pub use spec::GateSpec;
pub use verdict::{FailReason, GateResult, Verdict};
