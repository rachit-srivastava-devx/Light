//! `MeterStore` fakes shared by the `admit`/`settle` integration tests.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use fleet_govern::{LaneState, MeterIoError, MeterStore};
use fleet_types::{LaneId, Tokens};

pub struct InMemoryStore {
    pub lanes: Mutex<BTreeMap<String, LaneState>>,
}

impl InMemoryStore {
    pub fn new() -> Self {
        Self { lanes: Mutex::new(BTreeMap::new()) }
    }

    pub fn seed(&self, lane: &LaneId, state: LaneState) {
        self.lanes.lock().unwrap().insert(lane.as_str().to_string(), state);
    }
}

impl MeterStore for InMemoryStore {
    fn with_lane_locked(&self, lane: &LaneId, f: &mut dyn FnMut(&mut Option<LaneState>)) -> Result<(), MeterIoError> {
        let mut lanes = self.lanes.lock().unwrap();
        let mut slot = lanes.get(lane.as_str()).cloned();
        f(&mut slot);
        match slot {
            Some(s) => lanes.insert(lane.as_str().to_string(), s),
            None => lanes.remove(lane.as_str()),
        };
        Ok(())
    }

    fn snapshot_remaining(&self) -> Result<BTreeMap<String, Option<Tokens>>, MeterIoError> {
        let lanes = self.lanes.lock().unwrap();
        Ok(lanes
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

pub struct FailingPublishStore {
    pub inner: InMemoryStore,
    pub fail: AtomicBool,
}

impl MeterStore for FailingPublishStore {
    fn with_lane_locked(&self, lane: &LaneId, f: &mut dyn FnMut(&mut Option<LaneState>)) -> Result<(), MeterIoError> {
        if self.fail.load(Ordering::SeqCst) {
            let mut slot = self.inner.lanes.lock().unwrap().get(lane.as_str()).cloned();
            f(&mut slot); // mutation computed, then discarded -- publish never happens.
            return Err(MeterIoError("simulated publish failure".into()));
        }
        self.inner.with_lane_locked(lane, f)
    }

    fn snapshot_remaining(&self) -> Result<BTreeMap<String, Option<Tokens>>, MeterIoError> {
        self.inner.snapshot_remaining()
    }
}
