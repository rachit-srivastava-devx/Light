//! Orchestration -- the only two fns that touch the injected traits.

use crate::classify::classify;
use crate::ports::{ProcessRunner, ToolProbe};
use crate::report::Report;
use crate::spec::GateSpec;
use crate::verdict::{GateResult, Verdict};

/// Run one gate to completion: probe -> (skip | run -> classify). Never panics; every branch
/// produces a `GateResult`.
pub fn run_gate(spec: &GateSpec, probe: &dyn ToolProbe, runner: &dyn ProcessRunner) -> GateResult {
    if !probe.available(spec.probe) {
        return GateResult {
            id: spec.id,
            verdict: Verdict::Skip {
                reason: format!("{} unavailable", spec.id),
                was_required: spec.requirement == crate::requirement::Requirement::Required,
            },
        };
    }
    let out = runner.run(spec.command);
    GateResult {
        id: spec.id,
        verdict: classify(spec, &out),
    }
}

/// Run every committed gate in order, in a fresh `Report`. Gates are independent -- nothing here
/// assumes gate N's outcome affects gate N+1's inputs.
pub fn run_all(specs: &[GateSpec], probe: &dyn ToolProbe, runner: &dyn ProcessRunner) -> Report {
    Report {
        results: specs.iter().map(|s| run_gate(s, probe, runner)).collect(),
    }
}
