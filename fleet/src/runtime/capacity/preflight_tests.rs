//! One test per `CapacityRefusal` variant, each asserting the specific variant fired and that
//! its message carries the measured numbers -- plus the two happy-path shapes.

use super::*;

struct FakeProbe(fn() -> Result<Measurement, ProbeError>);

impl CapacityProbe for FakeProbe {
    fn measure(&self) -> Result<Measurement, ProbeError> {
        (self.0)()
    }
}

fn cfg() -> PreflightConfig {
    PreflightConfig { review_cap: 3, load_factor: DEFAULT_LOAD_FACTOR, per_lane_budget_bytes: PER_LANE_BUDGET_BYTES, ram_lanes_ceiling: None }
}

fn healthy(cores: usize, available_bytes: u64, load: f64) -> Measurement {
    Measurement { total_memory_bytes: available_bytes * 2, available_memory_bytes: available_bytes, load_avg_1m: load, logical_cores: cores }
}

#[test]
fn allows_a_healthy_machine_and_still_binds_on_review_cap() {
    let probe = FakeProbe(|| Ok(healthy(8, 8 * PER_LANE_BUDGET_BYTES, 1.0)));
    let cap = preflight(&probe, &cfg()).expect("healthy machine is allowed");
    assert_eq!(cap.get(), 3); // cores-2=6, ram_lanes=8, review_cap=3 binds
}

#[test]
fn refuses_on_insufficient_memory_with_measured_numbers_in_the_message() {
    let probe = FakeProbe(|| Ok(healthy(8, 500 * 1024 * 1024, 1.0)));
    let err = preflight(&probe, &cfg()).unwrap_err();
    assert!(matches!(err, CapacityRefusal::InsufficientMemory { available_mb: 500, budget_mb: 2048 }));
    let msg = err.to_string();
    assert!(msg.contains("500"), "message should carry measured available MiB: {msg}");
    assert!(msg.contains("2048"), "message should carry the budget MiB: {msg}");
}

#[test]
fn refuses_on_overloaded_machine_with_measured_numbers_in_the_message() {
    let probe = FakeProbe(|| Ok(healthy(4, 8 * PER_LANE_BUDGET_BYTES, 20.0)));
    let err = preflight(&probe, &cfg()).unwrap_err();
    assert!(matches!(err, CapacityRefusal::Overloaded { cores: 4, .. }));
    let msg = err.to_string();
    assert!(msg.contains("20.00"), "message should carry the measured load: {msg}");
    assert!(msg.contains('4'), "message should carry the core count: {msg}");
}

#[test]
fn refuses_when_the_probe_itself_cannot_measure() {
    let probe = FakeProbe(|| Err(ProbeError::SourceUnavailable { resource: "vm_stat", detail: "not found".into() }));
    let err = preflight(&probe, &cfg()).unwrap_err();
    assert!(matches!(err, CapacityRefusal::ProbeFailed(_)));
    assert!(err.to_string().contains("vm_stat"), "unknown capacity must say what failed: {err}");
}

#[test]
fn an_operator_ram_lanes_ceiling_can_only_lower_the_derived_cap() {
    let mut c = cfg();
    c.ram_lanes_ceiling = Some(1);
    let probe = FakeProbe(|| Ok(healthy(8, 8 * PER_LANE_BUDGET_BYTES, 1.0)));
    let cap = preflight(&probe, &c).expect("healthy");
    assert_eq!(cap.get(), 1);
}
