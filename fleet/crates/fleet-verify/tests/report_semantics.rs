use fleet_types::ExitCode;
use fleet_verify::{Denominator, FailReason, GateResult, Report, Verdict};

fn pass(id: &'static str) -> GateResult {
    GateResult {
        id,
        verdict: Verdict::Pass(Denominator::new(1, 1).unwrap()),
    }
}

fn fail(id: &'static str) -> GateResult {
    GateResult {
        id,
        verdict: Verdict::Fail {
            reason: FailReason::NonZeroExit(1),
            denominator: None,
        },
    }
}

fn skip(id: &'static str, was_required: bool) -> GateResult {
    GateResult {
        id,
        verdict: Verdict::Skip {
            reason: "unavailable".into(),
            was_required,
        },
    }
}

#[test]
fn report_exit_code_ranks_env_fault_over_invariant_fail() {
    let report = Report {
        results: vec![skip("a", true), fail("b")],
    };
    assert_eq!(report.exit_code(), ExitCode::Env);
}

#[test]
fn report_counts_match_verify_sh_semantics() {
    let report = Report {
        results: vec![
            pass("p1"),
            pass("p2"),
            pass("p3"),
            fail("f1"),
            fail("f2"),
            skip("s1", true),
            skip("s2", false),
        ],
    };
    assert_eq!(report.passed(), 3);
    assert_eq!(report.failed(), 2);
    assert_eq!(report.skipped(), 2);
    assert_eq!(report.env_faults(), 1);
}

#[test]
fn report_all_pass_is_ok() {
    let report = Report {
        results: vec![pass("p1")],
    };
    assert_eq!(report.exit_code(), ExitCode::Ok);
}

#[test]
fn report_fail_without_env_fault_is_invariant() {
    let report = Report {
        results: vec![fail("f1")],
    };
    assert_eq!(report.exit_code(), ExitCode::Invariant);
}
