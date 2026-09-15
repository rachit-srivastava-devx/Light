use super::*;
use crate::{DenominatorResult, GateCommand, ProbeTool, ProcessOutput, Requirement};

struct Missing;
impl ToolProbe for Missing {
    fn available(&self, _: ProbeTool) -> bool {
        false
    }
    fn unavailable_reason(&self, _: ProbeTool) -> String {
        "tool missing".into()
    }
}

struct MustNotRun;
impl ProcessRunner for MustNotRun {
    fn run(&self, _: &[&str]) -> ProcessOutput {
        panic!("unavailable gates must not spawn");
    }
}

#[test]
fn unavailable_required_gate_is_a_visible_environment_skip() {
    let spec = GateSpec {
        id: "g",
        requirement: Requirement::Required,
        probe: ProbeTool::Cargo,
        command: GateCommand::OnPath(&["true"]),
        parse_denominator: |_, _| DenominatorResult::Counted(1, 1),
    };
    let gates = GatesRoot::materialize().expect("embedded gates");
    let result = run_gate(&spec, &Missing, &MustNotRun, &gates);
    assert!(matches!(result.verdict, Verdict::Skip {
        was_required: true, ref reason
    } if reason.contains("tool missing")));
}
