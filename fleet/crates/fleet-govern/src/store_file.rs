//! `FileMeterStore`: fs4 exclusive lock across read-check-mutate-publish, tmp-write-then-rename
//! durable publish -- the fix for `meter.rs::save`'s unconditional, lock-free rename. The OS
//! advisory lock protects other threads/processes going through this same trait impl only, not
//! another program editing the data file directly outside this crate.

use std::collections::BTreeMap;
use std::fs::{self};
use std::io::{ErrorKind, Write};
use std::path::PathBuf;

use fleet_types::{LaneId, Tokens};

use crate::file_lock::with_exclusive_lock;
use crate::meter_codec::{decode_lanes, encode_lanes};
use crate::store::MeterStore;
use crate::types::{LaneState, MeterIoError};

pub struct FileMeterStore {
    path: PathBuf,
}

impl FileMeterStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn read_lanes(&self) -> Result<BTreeMap<String, LaneState>, MeterIoError> {
        match fs::read_to_string(&self.path) {
            Ok(text) => decode_lanes(&text),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(BTreeMap::new()),
            Err(e) => Err(MeterIoError(format!("read {}: {e}", self.path.display()))),
        }
    }

    fn publish(&self, lanes: &BTreeMap<String, LaneState>) -> Result<(), MeterIoError> {
        let tmp = self.path.with_extension("tmp");
        let mut file = fs::File::create(&tmp).map_err(|e| MeterIoError(format!("create tmp: {e}")))?;
        file.write_all(encode_lanes(lanes).as_bytes())
            .map_err(|e| MeterIoError(format!("write tmp: {e}")))?;
        file.sync_all().map_err(|e| MeterIoError(format!("sync tmp: {e}")))?;
        drop(file);
        fs::rename(&tmp, &self.path).map_err(|e| MeterIoError(format!("rename: {e}")))
    }
}

impl MeterStore for FileMeterStore {
    fn with_lane_locked(
        &self,
        lane: &LaneId,
        f: &mut dyn FnMut(&mut Option<LaneState>),
    ) -> Result<(), MeterIoError> {
        with_exclusive_lock(&self.path, || {
            let mut lanes = self.read_lanes()?;
            let mut slot = lanes.get(lane.as_str()).cloned();
            f(&mut slot);
            match slot {
                Some(state) => lanes.insert(lane.as_str().to_string(), state),
                None => lanes.remove(lane.as_str()),
            };
            self.publish(&lanes)
        })
    }

    fn snapshot_remaining(&self) -> Result<BTreeMap<String, Option<Tokens>>, MeterIoError> {
        let lanes = self.read_lanes()?;
        Ok(lanes
            .into_iter()
            .map(|(k, v)| {
                let remaining = match (v.window, v.used) {
                    (Some(w), Some(u)) => w.checked_sub(u).ok(),
                    _ => None,
                };
                (k, remaining)
            })
            .collect())
    }
}
