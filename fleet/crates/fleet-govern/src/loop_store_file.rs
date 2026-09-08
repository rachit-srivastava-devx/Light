//! `FileLoopStore`: one durable file holding every plan's progress, one line per plan (codec in
//! `loop_row_codec.rs`). Same lock + tmp-write-then-rename publish pattern as
//! `FileMeterStore`/`FileCooldownStore`, reused via `file_lock::with_exclusive_lock`.

use std::collections::BTreeMap;
use std::fs::{self};
use std::io::{ErrorKind, Write};
use std::path::PathBuf;

use crate::file_lock::with_exclusive_lock;
use crate::loop_row_codec::{encode_row, parse_row};
use crate::loop_store::LoopStore;
use crate::loop_types::{LoopIoError, LoopProgress};
use crate::types::MeterIoError;

pub struct FileLoopStore {
    path: PathBuf,
}

impl FileLoopStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn read_all(&self) -> Result<BTreeMap<String, LoopProgress>, LoopIoError> {
        match fs::read_to_string(&self.path) {
            Ok(text) => Ok(text.lines().filter_map(parse_row).collect()),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(BTreeMap::new()),
            Err(e) => Err(LoopIoError(format!("read {}: {e}", self.path.display()))),
        }
    }

    fn write_all(&self, rows: &BTreeMap<String, LoopProgress>) -> Result<(), LoopIoError> {
        let tmp = self.path.with_extension("tmp");
        let text: String = rows.iter().map(|(id, p)| encode_row(id, p)).collect();
        let mut file = fs::File::create(&tmp).map_err(|e| LoopIoError(format!("create tmp: {e}")))?;
        file.write_all(text.as_bytes()).map_err(|e| LoopIoError(format!("write tmp: {e}")))?;
        file.sync_all().map_err(|e| LoopIoError(format!("sync tmp: {e}")))?;
        drop(file);
        fs::rename(&tmp, &self.path).map_err(|e| LoopIoError(format!("rename: {e}")))
    }
}

impl LoopStore for FileLoopStore {
    fn load(&self, plan_id: &str) -> Result<Option<LoopProgress>, LoopIoError> {
        Ok(self.read_all()?.remove(plan_id))
    }

    fn save(&self, plan_id: &str, progress: &LoopProgress) -> Result<(), LoopIoError> {
        // `file_lock`'s helper is typed over `MeterIoError`; wrap/unwrap so this store gets the
        // same real OS advisory lock without duplicating the lock-acquisition logic.
        with_exclusive_lock(&self.path, || {
            let mut rows = self.read_all().map_err(|e| MeterIoError(e.0))?;
            rows.insert(plan_id.to_string(), progress.clone());
            self.write_all(&rows).map_err(|e| MeterIoError(e.0))
        })
        .map_err(|e| LoopIoError(e.0))
    }
}
