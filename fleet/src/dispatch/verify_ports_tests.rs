//! The starvation property behind `per_gate_budget`. Split out of `verify_ports.rs` for the
//! 80-line cap, matching `verify_cmd_tests.rs`/`verify_report_tests.rs`.

use super::*;

/// Gate #1 taking its entire slice must leave the overall deadline with time still on it, so
/// gate #2 gets a real chance to run rather than the instant 124 that made `fleet run` report
/// seven bogus timeouts behind one genuinely slow `cargo test`.
#[test]
fn one_gate_cannot_consume_the_whole_invocation_budget() {
    let total = Duration::from_secs(300);
    assert!(per_gate_budget(total) < total, "a gate must not be able to spend the total");
    let left = total - per_gate_budget(total);
    assert!(left >= Duration::from_secs(1), "starved gates need real time, got {left:?}");
}

/// The short budgets the real-binary tests use must still yield a nonzero per-gate slice --
/// a zero slice would make `run_bounded` report `budget_spent` without ever spawning, turning
/// those timeout tests green for the wrong reason.
#[test]
fn short_test_budgets_still_leave_a_spawnable_slice() {
    for secs in [1u64, 2, 60] {
        let slice = per_gate_budget(Duration::from_secs(secs));
        assert!(!slice.is_zero(), "{secs}s total gave a zero per-gate slice");
    }
}

/// The default is what `fleet run` actually gets with no env set. Pinned because the old 8s
/// default was the whole defect: no real `cargo test` on a cold repo fits in it.
#[test]
fn the_default_total_budget_can_fit_a_real_cargo_test() {
    let d = default_total_budget();
    assert!(d >= Duration::from_secs(120), "got {d:?}");
    assert!(!per_gate_budget(d).is_zero());
}
