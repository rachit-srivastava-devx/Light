//! `MeterStore`/`CooldownStore` fakes shared by the failover-pipeline integration tests.

use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, SystemTime};

use fleet_govern::{CooldownStore, LaneState, MeterIoError, MeterStore};
use fleet_types::{LaneId, Tokens};

pub struct FixedStore {
    pub lanes: BTreeMap<String, LaneState>,
}

impl MeterStore for FixedStore {
    fn with_lane_locked(&self, _lane: &LaneId, _f: &mut dyn FnMut(&mut Option<LaneState>)) -> Result<(), MeterIoError> {
        unreachable!("next_provider must never mutate the ledger")
    }

    fn snapshot_remaining(&self) -> Result<BTreeMap<String, Option<Tokens>>, MeterIoError> {
        Ok(self
            .lanes
            .iter()
            .map(|(k, v)| {
                let r = match (v.window, v.used) {
                    (Some(w), Some(u)) => w.checked_sub(u).ok(),
                    _ => None,
                };
                (k.clone(), r)
            })
            .collect())
    }
}

pub struct FixedCooldown {
    pub cooling: BTreeSet<&'static str>,
}

impl CooldownStore for FixedCooldown {
    fn start(&self, _adapter: &str, _at: SystemTime, _duration: Duration) -> Result<(), MeterIoError> {
        unreachable!("next_provider must never start a cooldown")
    }

    fn is_cooling_down(&self, adapter: &str, _now: SystemTime) -> Result<bool, MeterIoError> {
        Ok(self.cooling.contains(adapter))
    }
}
