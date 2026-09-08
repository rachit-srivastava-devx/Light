//! Shared test fakes/helpers for the `fleet-govern` integration tests. Each test binary pulls in
//! only the subset it needs -- unused items are expected, hence the blanket allow below.
#![allow(dead_code, unused_imports)]

mod failover_store;
mod meter_store;

pub use failover_store::*;
pub use meter_store::*;

use fleet_govern::LaneState;
use fleet_types::{LaneId, Tokens};

pub fn lane(name: &str) -> LaneId {
    LaneId::parse(name).unwrap()
}

pub fn measured(window: u64, used: u64) -> LaneState {
    LaneState { window: Some(Tokens::new(window)), used: Some(Tokens::new(used)), ..Default::default() }
}
