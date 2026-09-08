//! Unit-level coverage for `LifecycleState`/`GateRefusal`/`ExitCode` (BLUEPRINT.md §9), run as
//! integration tests so the src files stay thin per the 80-line rule.

use fleet_types::{ExitCode, GateRefusal, LifecycleState, UnknownExitCode};

#[test]
fn allowed_next_is_total_and_matches_table() {
    assert_eq!(
        LifecycleState::Building.allowed_next(),
        &[LifecycleState::Built, LifecycleState::Refused]
    );
    assert_eq!(LifecycleState::Refused.allowed_next(), &[] as &[LifecycleState]);
    assert_eq!(LifecycleState::Observed.allowed_next(), &[LifecycleState::Intake]);
}

#[test]
fn gate_refusal_accessors() {
    let refusal = GateRefusal::new("EMPTY_TASK_ID", "a task id must not be empty");
    assert_eq!(refusal.code(), "EMPTY_TASK_ID");
    assert_eq!(refusal.message(), "a task id must not be empty");
}

#[test]
fn try_from_matches_only_the_six_known_values() {
    for (raw, code) in [
        (0, ExitCode::Ok),
        (3, ExitCode::Env),
        (6, ExitCode::Invariant),
        (7, ExitCode::Refusal),
        (8, ExitCode::Mismatch),
        (9, ExitCode::ReadyAwaitingReview),
    ] {
        assert_eq!(ExitCode::try_from(raw), Ok(code));
        assert_eq!(code.as_i32(), raw);
    }
    assert_eq!(ExitCode::try_from(4), Err(UnknownExitCode(4)));
    assert_eq!(ExitCode::try_from(-1), Err(UnknownExitCode(-1)));
}
