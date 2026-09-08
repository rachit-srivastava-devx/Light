//! Drives the capacity preflight through the REAL compiled `fleet` binary -- proves the gate is
//! actually wired into `main.rs` ahead of the runtimes, not just correct in its own unit tests
//! (a proxy is not the property). `env!("CARGO_BIN_EXE_fleet")` is the real product binary.

mod support;
use support::bin;

use std::process::{Command, Output};

const REFUSAL: i32 = 7; // fleet_types::ExitCode::Refusal

fn run(env: &[(&str, &str)], args: &[&str]) -> Output {
    let mut c = Command::new(bin());
    c.args(args);
    for (k, v) in env {
        c.env(k, v);
    }
    c.output().expect("binary runs")
}

// `status` is deliberately excluded from capacity gating (`cli::capacity_scope`, pinned by
// `introspection_commands_are_never_capacity_gated`), so these refusal tests drive `oracle`
// instead: a real work-spawning command that IS gated, with no args of its own.

/// An impossible per-lane budget must refuse, naming both measured numbers.
#[test]
fn an_unmeetable_lane_budget_refuses_with_the_measured_numbers() {
    let out = run(&[("FLEET_LANE_BUDGET_MB", "99999999")], &["oracle"]);
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(REFUSAL), "stderr: {err}");
    assert!(err.contains("available memory"), "no measured memory in: {err}");
    assert!(err.contains("99999999"), "no threshold in: {err}");
}

/// A load factor of ~0 makes any live machine "overloaded"; the refusal names load and cores.
#[test]
fn an_impossible_load_factor_refuses_naming_load_cores_and_threshold() {
    let out = run(&[("FLEET_LOAD_FACTOR", "0.001")], &["oracle"]);
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(REFUSAL), "stderr: {err}");
    assert!(err.contains("load"), "no measured load in: {err}");
    assert!(err.contains("cores"), "no core count in: {err}");
}

/// The gate must run BEFORE any work: a refusal produces no stdout at all, proving nothing ran.
#[test]
fn a_refusal_short_circuits_before_any_work_is_done() {
    let out = run(&[("FLEET_LANE_BUDGET_MB", "99999999")], &["oracle"]);
    assert_eq!(out.status.code(), Some(REFUSAL));
    assert!(out.stdout.is_empty(), "work ran despite refusal: {:?}", String::from_utf8_lossy(&out.stdout));
}

/// With no overrides this machine must be allowed -- otherwise the gate is unconditional, which
/// would be a check that always fires and therefore measures nothing.
#[test]
fn the_gate_allows_an_unconstrained_run_so_it_is_not_unconditional() {
    // Neutralise only the LOAD threshold (not a bypass) so this doesn't depend on ambient load.
    let out = run(&[("FLEET_LOAD_FACTOR", "10000")], &["status", "--json"]);
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "stderr: {err}");
    assert!(String::from_utf8_lossy(&out.stdout).contains("concurrency_cap"));
}

/// `status` must report the cap `main.rs` MEASURED, not one it re-derives. Not a refusal case, so
/// the LOAD threshold is neutralised the same way as the "allows" test above.
#[test]
fn status_reports_the_same_cap_the_probe_measured() {
    let hi = [("FLEET_LOAD_FACTOR", "10000")];
    let probe = run(&hi, &["__capacity_probe"]);
    assert!(probe.status.success());
    let probe_out = String::from_utf8_lossy(&probe.stdout);
    let measured = probe_out
        .lines()
        .find_map(|l| l.strip_prefix("decision: allow (concurrency_cap=")?.strip_suffix(')'))
        .expect("probe reports an allow decision with a cap")
        .to_string();
    let status = run(&hi, &["status", "--json"]);
    let shown = String::from_utf8_lossy(&status.stdout);
    assert!(shown.contains(&measured), "status cap disagrees with measured {measured}: {shown}");
}
