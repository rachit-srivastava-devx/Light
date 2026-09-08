//! Orchestration -- the only two fns that touch the injected traits.

use crate::classify::classify;
use crate::gates::GatesRoot;
use crate::ports::{ProcessRunner, ToolProbe};
use crate::report::Report;
use crate::requirement::Requirement;
use crate::spec::{GateCommand, GateSpec};
use crate::verdict::{GateResult, Verdict};

fn skip(spec: &GateSpec, reason: String) -> GateResult {
    GateResult {
        id: spec.id,
        verdict: Verdict::Skip { reason, was_required: spec.requirement == Requirement::Required },
    }
}

/// Build this gate's argv, resolving a `Script` variant's path against `gates` first. Returns
/// owned strings -- a resolved script path cannot be `'static`.
fn resolve_argv(spec: &GateSpec, gates: &GatesRoot) -> Result<Vec<String>, GateResult> {
    match spec.command {
        GateCommand::OnPath(args) => Ok(args.iter().map(|s| s.to_string()).collect()),
        GateCommand::Script { relative, args } => match gates.require(relative) {
            Ok(path) => {
                let mut argv = vec![path.to_string_lossy().into_owned()];
                argv.extend(args.iter().map(|s| s.to_string()));
                Ok(argv)
            }
            Err(e) => Err(skip(spec, format!("{} gate asset unavailable: {e}", spec.id))),
        },
    }
}

/// Run one gate to completion: probe -> (skip | resolve -> run -> classify). Never panics; every
/// branch produces a `GateResult`.
pub fn run_gate(
    spec: &GateSpec,
    probe: &dyn ToolProbe,
    runner: &dyn ProcessRunner,
    gates: &GatesRoot,
) -> GateResult {
    if !probe.available(spec.probe) {
        return skip(spec, format!("{} unavailable", spec.id));
    }
    let argv = match resolve_argv(spec, gates) {
        Ok(argv) => argv,
        Err(result) => return result,
    };
    let argv: Vec<&str> = argv.iter().map(String::as_str).collect();
    let out = runner.run(&argv);
    GateResult { id: spec.id, verdict: classify(spec, &out) }
}

/// Run every committed gate in order, in a fresh `Report`. Gates are independent -- nothing here
/// assumes gate N's outcome affects gate N+1's inputs.
pub fn run_all(
    specs: &[GateSpec],
    probe: &dyn ToolProbe,
    runner: &dyn ProcessRunner,
    gates: &GatesRoot,
) -> Report {
    Report {
        results: specs.iter().map(|s| run_gate(s, probe, runner, gates)).collect(),
    }
}
