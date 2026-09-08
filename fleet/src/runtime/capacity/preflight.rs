//! The preflight: wraps `ConcurrencyCap::compute` with a real measurement and a hard refusal
//! path -- unknown or saturated capacity is not permission to proceed. `compute()` stays pure
//! and total; this is the only place allowed to say no. Tests: `preflight_tests.rs`.

use super::lanes::{ram_lanes_from_available, PER_LANE_BUDGET_BYTES};
use super::measurement::{CapacityProbe, ProbeError};
#[cfg(test)]
use super::measurement::Measurement;
use crate::runtime::ConcurrencyCap;

/// Refuse once 1-minute load exceeds cores * this factor. Override via `FLEET_LOAD_FACTOR`.
pub const DEFAULT_LOAD_FACTOR: f64 = 2.0;

#[derive(Clone, Copy, Debug)]
pub struct PreflightConfig {
    pub review_cap: usize,
    pub load_factor: f64,
    pub per_lane_budget_bytes: u64,
    /// An operator-supplied ceiling on `ram_lanes` (e.g. `FLEET_RAM_LANES`), applied on top of
    /// the measured value -- it can only lower the derived lanes, never inflate them.
    pub ram_lanes_ceiling: Option<usize>,
}

impl PreflightConfig {
    pub fn from_env(review_cap: usize, ram_lanes_ceiling: Option<usize>) -> Self {
        let load_factor = env_f64("FLEET_LOAD_FACTOR").unwrap_or(DEFAULT_LOAD_FACTOR);
        let per_lane_budget_bytes = env_f64("FLEET_LANE_BUDGET_MB")
            .map(|mb| (mb * 1024.0 * 1024.0) as u64)
            .unwrap_or(PER_LANE_BUDGET_BYTES);
        Self { review_cap, load_factor, per_lane_budget_bytes, ram_lanes_ceiling }
    }
}

fn env_f64(key: &str) -> Option<f64> {
    std::env::var(key).ok()?.trim().parse().ok()
}

/// One named variant per refusal reason, each carrying the measured numbers that triggered it.
#[derive(Debug, thiserror::Error)]
pub enum CapacityRefusal {
    #[error("capacity probe failed to measure the machine: {0}")]
    ProbeFailed(#[from] ProbeError),
    #[error("available memory {available_mb} MiB is below one lane's budget of {budget_mb} MiB")]
    InsufficientMemory { available_mb: u64, budget_mb: u64 },
    #[error("1-minute load {load:.2} exceeds {cores} cores x {factor} = {threshold:.2}")]
    Overloaded { load: f64, cores: usize, factor: f64, threshold: f64 },
}

pub fn preflight(probe: &dyn CapacityProbe, cfg: &PreflightConfig) -> Result<ConcurrencyCap, CapacityRefusal> {
    let m = probe.measure()?;
    if m.available_memory_bytes < cfg.per_lane_budget_bytes {
        return Err(CapacityRefusal::InsufficientMemory {
            available_mb: m.available_memory_bytes / (1024 * 1024),
            budget_mb: cfg.per_lane_budget_bytes / (1024 * 1024),
        });
    }
    let threshold = m.logical_cores as f64 * cfg.load_factor;
    if m.load_avg_1m > threshold {
        return Err(CapacityRefusal::Overloaded { load: m.load_avg_1m, cores: m.logical_cores, factor: cfg.load_factor, threshold });
    }
    let mut ram_lanes = ram_lanes_from_available(m.available_memory_bytes, cfg.per_lane_budget_bytes);
    if let Some(ceiling) = cfg.ram_lanes_ceiling {
        ram_lanes = ram_lanes.min(ceiling);
    }
    Ok(ConcurrencyCap::compute(m.logical_cores, ram_lanes, cfg.review_cap))
}

#[cfg(test)]
#[path = "preflight_tests.rs"]
mod tests;
