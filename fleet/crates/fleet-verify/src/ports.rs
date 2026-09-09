//! Injected IO boundaries: `ToolProbe` and `ProcessRunner`. This crate never calls
//! `std::process::Command` or `command -v` itself -- both are supplied by the caller.

use crate::requirement::ProbeTool;

/// Injected IO boundary #1: "is this tool usable on this machine right now."
pub trait ToolProbe {
    fn available(&self, tool: ProbeTool) -> bool;

    /// Why `tool` is unavailable, for the gate's skip line -- only ever consulted after
    /// `available` returned `false`. Two real causes read very differently to a user ("this
    /// isn't opted in" vs. "this isn't installed"), but a probe with only one cause (or one
    /// that hasn't been taught to distinguish yet) can rely on this default, which reproduces
    /// the old collapsed "unavailable" text exactly.
    fn unavailable_reason(&self, tool: ProbeTool) -> String {
        let _ = tool;
        "unavailable".to_string()
    }
}

/// The raw result of running one gate's command line. `exit_code` is the wrapped script's own
/// process exit status -- this crate does not normalize it, it only asks "was it zero."
#[derive(Clone, Debug)]
pub struct ProcessOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Injected IO boundary #2: actually run a gate's argv.
pub trait ProcessRunner {
    fn run(&self, command: &[&str]) -> ProcessOutput;
}
