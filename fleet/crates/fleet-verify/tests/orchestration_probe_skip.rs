mod support;

use fleet_verify::{DenominatorResult, GateSpec, ProbeTool, Requirement, Verdict};
use support::{out, FakeProbe, FakeRunner};

fn always_pass(_stdout: &str, _stderr: &str) -> DenominatorResult {
    DenominatorResult::Counted(1, 1)
}

fn gate(id: &'static str, requirement: Requirement) -> GateSpec {
    GateSpec {
        id,
        requirement,
        probe: ProbeTool::Named("missing-tool"),
        command: &["some-cmd"],
        parse_denominator: always_pass,
    }
}

fn empty_probe() -> FakeProbe {
    FakeProbe {
        available: Default::default(),
    }
}

#[test]
fn required_tool_missing_produces_env_fault_skip() {
    let spec = gate("g", Requirement::Required);
    let runner = FakeRunner::new(vec![]);
    let result = fleet_verify::run_gate(&spec, &empty_probe(), &runner);
    match result.verdict {
        Verdict::Skip { was_required: true, .. } => {}
        other => panic!("expected Skip{{was_required: true}}, got {other:?}"),
    }
    assert!(runner.calls.borrow().is_empty(), "process must never run");
}

#[test]
fn advisory_tool_missing_produces_plain_skip() {
    let spec = gate("g", Requirement::Advisory);
    let runner = FakeRunner::new(vec![]);
    let result = fleet_verify::run_gate(&spec, &empty_probe(), &runner);
    match result.verdict {
        Verdict::Skip { was_required: false, .. } => {}
        other => panic!("expected Skip{{was_required: false}}, got {other:?}"),
    }
    assert!(runner.calls.borrow().is_empty(), "process must never run");
}

#[test]
fn run_all_preserves_spec_order_and_runs_every_gate_independently() {
    let mut specs = Vec::new();
    for i in 0..5 {
        specs.push(GateSpec {
            id: Box::leak(format!("gate-{i}").into_boxed_str()),
            requirement: Requirement::Required,
            probe: ProbeTool::Cargo,
            command: &["cmd"],
            parse_denominator: always_pass,
        });
    }
    let probe = FakeProbe {
        available: [ProbeTool::Cargo].into_iter().collect(),
    };
    let runner = FakeRunner::new(vec![
        out(0, ""),
        out(1, "boom"),
        out(0, ""),
        out(0, ""),
        out(0, ""),
    ]);
    let report = fleet_verify::run_all(&specs, &probe, &runner);
    assert_eq!(report.results.len(), 5);
    assert_eq!(report.results[0].id, "gate-0");
    assert_eq!(report.results[4].id, "gate-4");
    assert_eq!(runner.calls.borrow().len(), 5, "every gate must run independently");
}
