//! Shared test fakes/helpers for the `fleet-govern` integration tests. Each test binary pulls in
//! only the subset it needs -- unused items are expected, hence the blanket allow below.
#![allow(dead_code, unused_imports)]

mod failover_store;
mod loop_store;
mod meter_store;

pub use failover_store::*;
pub use loop_store::*;
pub use meter_store::*;

use fleet_govern::{EscalationPolicy, LaneState, LoopPlan, UnitId};
use fleet_router::TaskClass;
use fleet_types::{LaneId, Role, Tokens};

pub fn lane(name: &str) -> LaneId {
    LaneId::parse(name).unwrap()
}

pub fn measured(window: u64, used: u64) -> LaneState {
    LaneState { window: Some(Tokens::new(window)), used: Some(Tokens::new(used)), ..Default::default() }
}

/// A `LoopPlan` of the given unit names, routed as `Role::Builder`/`TaskClass::Implementation`
/// with a fixed per-unit token estimate -- the shape every `AutonomousRun` test needs.
pub fn plan(id: &str, units: &[&str], tokens_per_unit: u64) -> LoopPlan {
    LoopPlan {
        id: id.into(),
        units: units.iter().map(|u| UnitId::new(*u)).collect(),
        role: Some(Role::Builder),
        class: TaskClass::Implementation,
        tokens_per_unit: Tokens::new(tokens_per_unit),
    }
}

/// A ladder that never fires `Escalation::Pause` -- for tests exercising `next_provider`'s own
/// quota-exhaustion switching rather than the escalation ladder itself.
pub fn policy_never_pauses() -> EscalationPolicy {
    let never = Tokens::new(u64::MAX);
    EscalationPolicy { throttle_at: never, downgrade_at: never, cached_at: never, pause_at: never }
}
