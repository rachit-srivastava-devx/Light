use super::*;
use crate::impl_::denominator::Denominator;
use crate::impl_::verdict::FailReason;
use types::ExitCode;

fn gate(verdict: Verdict) -> GateResult {
    GateResult { id: "g", verdict }
}

#[test]
fn counts_pass_fail_and_skips_with_exit_precedence() {
    let pass = gate(Verdict::Pass(Denominator::new(1, 1).unwrap()));
    let fail = gate(Verdict::Fail {
        reason: FailReason::MeasuredNothing,
        denominator: None,
    });
    let required = gate(Verdict::Skip {
        reason: "missing".into(),
        was_required: true,
    });
    let advisory = gate(Verdict::Skip {
        reason: "optional".into(),
        was_required: false,
    });
    let report = Report {
        results: vec![pass, fail, required, advisory],
    };
    assert_eq!(report.passed(), 1);
    assert_eq!(report.failed(), 1);
    assert_eq!(report.skipped(), 2);
    assert_eq!(report.env_faults(), 1);
    assert_eq!(report.exit_code(), ExitCode::Env);
}

#[test]
fn invariant_failure_is_used_without_environment_fault() {
    let report = Report {
        results: vec![gate(Verdict::Fail {
            reason: FailReason::NonZeroExit(1),
            denominator: None,
        })],
    };
    assert_eq!(report.exit_code(), ExitCode::Invariant);
}

#[test]
fn empty_report_has_zero_counts_and_is_an_invariant_failure() {
    let report = Report { results: vec![] };
    assert_eq!(report.passed(), 0);
    assert_eq!(report.failed(), 0);
    assert_eq!(report.skipped(), 0);
    assert_eq!(report.env_faults(), 0);
    assert_eq!(report.exit_code(), ExitCode::Invariant);
}
