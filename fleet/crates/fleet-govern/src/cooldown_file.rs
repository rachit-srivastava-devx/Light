//! `FileCooldownStore`: a tiny `adapter -> (start_epoch_secs, duration_secs)` table, the same
//! lock+tmp-rename publish pattern as `FileMeterStore`.

use std::collections::BTreeMap;
use std::fs::{self};
use std::io::{ErrorKind, Write};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::file_lock::with_exclusive_lock;
use crate::store::CooldownStore;
use crate::types::MeterIoError;

pub struct FileCooldownStore {
    path: PathBuf,
}

impl FileCooldownStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn read(&self) -> Result<BTreeMap<String, (u64, u64)>, MeterIoError> {
        match fs::read_to_string(&self.path) {
            Ok(text) => Ok(text.lines().filter_map(parse_row).collect()),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(BTreeMap::new()),
            Err(e) => Err(MeterIoError(format!("read {}: {e}", self.path.display()))),
        }
    }

    fn write(&self, table: &BTreeMap<String, (u64, u64)>) -> Result<(), MeterIoError> {
        let tmp = self.path.with_extension("tmp");
        let text: String = table.iter().map(|(a, (s, d))| format!("{a}\t{s}\t{d}\n")).collect();
        let mut file = fs::File::create(&tmp).map_err(|e| MeterIoError(format!("create: {e}")))?;
        file.write_all(text.as_bytes()).map_err(|e| MeterIoError(format!("write: {e}")))?;
        file.sync_all().map_err(|e| MeterIoError(format!("sync: {e}")))?;
        drop(file);
        fs::rename(&tmp, &self.path).map_err(|e| MeterIoError(format!("rename: {e}")))
    }
}

fn parse_row(line: &str) -> Option<(String, (u64, u64))> {
    let mut f = line.split('\t');
    Some((f.next()?.to_string(), (f.next()?.parse().ok()?, f.next()?.parse().ok()?)))
}

fn epoch_secs(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl CooldownStore for FileCooldownStore {
    fn start(&self, adapter: &str, at: SystemTime, duration: Duration) -> Result<(), MeterIoError> {
        with_exclusive_lock(&self.path, || {
            let mut table = self.read()?;
            table.insert(adapter.to_string(), (epoch_secs(at), duration.as_secs()));
            self.write(&table)
        })
    }

    fn is_cooling_down(&self, adapter: &str, now: SystemTime) -> Result<bool, MeterIoError> {
        let table = self.read()?;
        Ok(table.get(adapter).is_some_and(|(at, dur)| epoch_secs(now) < at + dur))
    }
}
