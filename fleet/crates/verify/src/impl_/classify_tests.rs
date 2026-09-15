use super::*;
use crate::{GateCommand, ProbeTool, Requirement};

fn spec() -> GateSpec {
    GateSpec {
        id: "unit",
        requirement: Requirement::Required,
        probe: ProbeTool::Cargo,
        command: GateCommand::OnPath(&["true"]),
        parse_denominator: |_, _| DenominatorResult::Counted(2, 3),
    }
}

#[test]
fn nonzero_exit_is_a_failure_with_its_measured_denominator() {
    let verdict = classify(
        &spec(),
        &ProcessOutput {
            exit_code: 6,
            stdout: String::new(),
            stderr: String::new(),
        },
    );
    assert!(matches!(verdict, Verdict::Fail {
        reason: FailReason::NonZeroExit(6),
        denominator: Some(d)
    } if d.numerator() == 2 && d.total() == 3));
}

#[test]
fn zero_total_is_measured_nothing_even_on_exit_zero() {
    let mut gate = spec();
    gate.parse_denominator = |_, _| DenominatorResult::Counted(0, 0);
    let verdict = classify(
        &gate,
        &ProcessOutput {
            exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
        },
    );
    assert!(matches!(
        verdict,
        Verdict::Fail {
            reason: FailReason::MeasuredNothing,
            denominator: None
        }
    ));
}
