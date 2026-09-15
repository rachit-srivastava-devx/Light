//! Orchestration -- the only two fns that touch the injected traits.

use super::gates::GatesRoot;
use super::ports::{ProcessRunner, ToolProbe};
use super::report::Report;
use super::requirement::Requirement;
use super::spec::{GateCommand, GateSpec};
use super::verdict::{GateResult, Verdict};

#[path = "orchestrate_output.rs"]
pub(crate) mod gate_output;

pub(super) fn skip(spec: &GateSpec, reason: String) -> GateResult {
    GateResult {
        id: spec.id,
        verdict: Verdict::Skip {
            reason,
            was_required: spec.requirement == Requirement::Required,
        },
    }
}

/// Build this gate's argv, resolving a `Script` variant's path against `gates` first. Returns
/// owned strings -- a resolved script path cannot be `'static`.
pub(super) fn resolve_argv(spec: &GateSpec, gates: &GatesRoot) -> Result<Vec<String>, GateResult> {
    match spec.command {
        GateCommand::OnPath(args) => Ok(args.iter().map(|s| s.to_string()).collect()),
        GateCommand::Script { relative, args } => match gates.require(relative) {
            Ok(path) => {
                let mut argv = vec![path.to_string_lossy().into_owned()];
                argv.extend(args.iter().map(|s| s.to_string()));
                Ok(argv)
            }
            Err(e) => Err(skip(
                spec,
                format!("{} gate asset unavailable: {e}", spec.id),
            )),
        },
    }
}

pub fn run_gate(
    spec: &GateSpec,
    probe: &dyn ToolProbe,
    runner: &dyn ProcessRunner,
    gates: &GatesRoot,
) -> GateResult {
    gate_output::run_gate_with_output(spec, probe, runner, gates).0
}

pub fn run_all(
    specs: &[GateSpec],
    probe: &dyn ToolProbe,
    runner: &dyn ProcessRunner,
    gates: &GatesRoot,
) -> Report {
    Report {
        results: specs
            .iter()
            .map(|s| run_gate(s, probe, runner, gates))
            .collect(),
    }
}

#[cfg(test)]
#[path = "orchestrate_tests.rs"]
mod tests;
