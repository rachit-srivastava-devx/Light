mod support;

use fleet_verify::{DenominatorResult, FailReason, GateSpec, ProbeTool, Requirement, Verdict};
use support::{dummy_gates, out, FakeProbe, FakeRunner};

fn caught_total(stdout: &str, _stderr: &str) -> DenominatorResult {
    let parse = |key: &str| -> Option<u64> {
        stdout.split_once(key)?.1.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().ok()
    };
    match (parse("caught="), parse("total=")) {
        (Some(c), Some(t)) => DenominatorResult::Counted(c, t),
        _ => DenominatorResult::Unparseable,
    }
}

fn spec() -> GateSpec {
    GateSpec {
        id: "test-gate",
        requirement: Requirement::Required,
        probe: ProbeTool::Cargo,
        command: fleet_verify::GateCommand::OnPath(&["true"]),
        parse_denominator: caught_total,
    }
}

fn probe() -> FakeProbe {
    FakeProbe { available: [ProbeTool::Cargo].into_iter().collect() }
}

#[test]
fn zero_exit_with_zero_total_is_fail_not_pass() {
    let runner = FakeRunner::new(vec![out(0, "caught=0 total=0")]);
    let result = fleet_verify::run_gate(&spec(), &probe(), &runner, &dummy_gates());
    assert!(matches!(
        result.verdict,
        Verdict::Fail { reason: FailReason::MeasuredNothing, .. }
    ));
}

#[test]
fn zero_exit_with_unrecognized_stdout_is_fail_unparseable() {
    let runner = FakeRunner::new(vec![out(0, "unexpected garbage")]);
    let result = fleet_verify::run_gate(&spec(), &probe(), &runner, &dummy_gates());
    assert!(matches!(result.verdict, Verdict::Fail { reason: FailReason::Unparseable, .. }));
}

#[test]
fn nonzero_exit_is_always_fail_even_with_a_valid_looking_denominator() {
    let runner = FakeRunner::new(vec![out(1, "caught=40 total=40")]);
    let result = fleet_verify::run_gate(&spec(), &probe(), &runner, &dummy_gates());
    assert!(matches!(
        result.verdict,
        Verdict::Fail { reason: FailReason::NonZeroExit(1), denominator: Some(_) }
    ));
}

#[test]
fn zero_exit_with_valid_denominator_is_pass() {
    let runner = FakeRunner::new(vec![out(0, "caught=40 total=40")]);
    let result = fleet_verify::run_gate(&spec(), &probe(), &runner, &dummy_gates());
    match result.verdict {
        Verdict::Pass(d) => {
            assert_eq!(d.numerator(), 40);
            assert_eq!(d.total(), 40);
        }
        other => panic!("expected Pass, got {other:?}"),
    }
}

#[test]
fn negative_exit_code_is_treated_as_nonzero_failure() {
    let runner = FakeRunner::new(vec![out(-1, "caught=1 total=1")]);
    let result = fleet_verify::run_gate(&spec(), &probe(), &runner, &dummy_gates());
    assert!(matches!(
        result.verdict,
        Verdict::Fail { reason: FailReason::NonZeroExit(-1), .. }
    ));
}
