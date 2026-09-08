//! `fleet __capacity_probe`: prints the real `CapacityProbe` reading plus whether the preflight
//! would allow or refuse a run, without spawning any lane. Also the shared report `doctor` (in
//! `ops_cmd.rs`) folds in -- see BLUEPRINT task "no crashing again": an operator must be able to
//! see the numbers *before* running work, not just after a refusal.

use crate::dispatch::error::DispatchError;
use crate::print::human;
use crate::runtime::capacity::{preflight, Measurement, PreflightConfig, StdCapacityProbe};

pub fn report(review_cap: usize, ram_lanes_ceiling: Option<usize>) -> Result<(), DispatchError> {
    let probe = StdCapacityProbe;
    let cfg = PreflightConfig::from_env(review_cap, ram_lanes_ceiling);
    match crate::runtime::capacity::CapacityProbe::measure(&probe) {
        Ok(m) => print_measurement(&m, cfg.per_lane_budget_bytes, cfg.load_factor),
        Err(e) => human::line("probe_error", e),
    }
    match preflight(&probe, &cfg) {
        Ok(cap) => human::line("decision", format!("allow (concurrency_cap={})", cap.get())),
        Err(refusal) => human::line("decision", format!("refuse: {refusal}")),
    }
    Ok(())
}

fn print_measurement(m: &Measurement, budget_bytes: u64, load_factor: f64) {
    human::line("total_memory_mb", m.total_memory_bytes / (1024 * 1024));
    human::line("available_memory_mb", m.available_memory_bytes / (1024 * 1024));
    human::line("load_avg_1m", m.load_avg_1m);
    human::line("logical_cores", m.logical_cores);
    human::line("per_lane_budget_mb", budget_bytes / (1024 * 1024));
    human::line("load_factor_threshold", load_factor);
    let derived = crate::runtime::capacity::ram_lanes_from_available(m.available_memory_bytes, budget_bytes);
    human::line("derived_ram_lanes", derived);
}
