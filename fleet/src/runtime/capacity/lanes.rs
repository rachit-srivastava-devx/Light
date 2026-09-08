//! Per-lane memory budget and the pure `ram_lanes` derivation from a real measurement.

/// Bytes budgeted per lane: one spawned CLI worker process (Claude/Codex/etc.) plus its git
/// worktree checkout. A CLI worker's steady-state RSS runs roughly 800MB-1.5GB and a worktree
/// checkout of this repo's size adds well under 200MB; 2 GiB leaves headroom for spikes without
/// being so conservative it starves small machines. Override per-run via `FLEET_LANE_BUDGET_MB`
/// (see `preflight.rs`) rather than editing this constant for one machine.
pub const PER_LANE_BUDGET_BYTES: u64 = 2 * 1024 * 1024 * 1024;

pub fn ram_lanes_from_available(available_memory_bytes: u64, per_lane_budget_bytes: u64) -> usize {
    if per_lane_budget_bytes == 0 {
        return 0;
    }
    (available_memory_bytes / per_lane_budget_bytes) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn divides_available_by_budget() {
        assert_eq!(ram_lanes_from_available(10 * PER_LANE_BUDGET_BYTES, PER_LANE_BUDGET_BYTES), 10);
        assert_eq!(ram_lanes_from_available(0, PER_LANE_BUDGET_BYTES), 0);
    }

    #[test]
    fn zero_budget_is_zero_lanes_not_a_divide_by_zero_panic() {
        assert_eq!(ram_lanes_from_available(1024, 0), 0);
    }
}
