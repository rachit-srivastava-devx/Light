use super::super::classify::classify;
use super::super::gates::GatesRoot;
use super::super::ports::{ProcessOutput, ProcessRunner, ToolProbe};
use super::super::spec::GateSpec;
use super::super::verdict::GateResult;
use super::{resolve_argv, skip};

pub(crate) fn run_gate_with_output(
    spec: &GateSpec,
    probe: &dyn ToolProbe,
    runner: &dyn ProcessRunner,
    gates: &GatesRoot,
) -> (GateResult, Option<ProcessOutput>) {
    if !probe.available(spec.probe) {
        return (
            skip(
                spec,
                format!("{} {}", spec.id, probe.unavailable_reason(spec.probe)),
            ),
            None,
        );
    }
    let argv = match resolve_argv(spec, gates) {
        Ok(argv) => argv,
        Err(result) => return (result, None),
    };
    let argv: Vec<&str> = argv.iter().map(String::as_str).collect();
    let out = runner.run(&argv);
    let result = GateResult {
        id: spec.id,
        verdict: classify(spec, &out),
    };
    (result, Some(out))
}
