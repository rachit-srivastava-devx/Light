//! Pins D1 (worst of the three E2E findings): a `Report` with at least one `Fail` verdict must
//! turn into an `Err(DispatchError)` whose `exit_code()` is non-zero. Before this fix,
//! `oracle`/`gate` printed the report and then unconditionally returned `Ok(())`, so a caller
//! doing `fleet gate || exit 1` could never observe a failing gate -- this must never come back.

use super::to_result;
use crate::dispatch::error::DispatchError;
use verify::{FailReason, GateResult, Report, Verdict};

fn fail_result(id: &'static str) -> GateResult {
    GateResult {
        id,
        verdict: Verdict::Fail {
            reason: FailReason::NonZeroExit(1),
            denominator: None,
        },
    }
}

/// A non-required skip does not contribute to `failed()` or `env_faults()`, so
/// `report.exit_code()` returns `Ok` in the absence of other failures — the same
/// observable outcome as a real `Verdict::Pass` from `to_result`'s perspective.
fn pass_result(id: &'static str) -> GateResult {
    GateResult {
        id,
        verdict: Verdict::Skip {
            reason: "not-applicable".into(),
            was_required: false,
        },
    }
}

#[test]
fn a_report_with_any_failing_gate_yields_a_nonzero_exit_code() {
    let report = Report {
        results: vec![pass_result("a"), fail_result("b")],
    };
    let err = to_result(report).expect_err("a failing gate must not be reported as Ok");
    assert_ne!(
        err.exit_code().as_i32(),
        0,
        "the whole point of this fix: never exit 0 on fail"
    );
    assert!(matches!(err, DispatchError::VerifyFailed { failed: 1, .. }));
}

#[test]
fn a_report_where_every_gate_passes_yields_ok() {
    let report = Report {
        results: vec![pass_result("a"), pass_result("b")],
    };
    assert!(to_result(report).is_ok());
}

#[test]
fn a_required_skip_also_yields_a_nonzero_exit_code() {
    let report = Report {
        results: vec![GateResult {
            id: "c",
            verdict: Verdict::Skip {
                reason: "tool unavailable".into(),
                was_required: true,
            },
        }],
    };
    let err = to_result(report).expect_err("a required, skipped gate must not report Ok");
    assert_ne!(err.exit_code().as_i32(), 0);
}
