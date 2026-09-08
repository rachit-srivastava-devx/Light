//! `FileCursorStore`: the real, durable `CursorStore` -- one small file per sink under a
//! directory. Crash-safe by construction: every write lands in a temp file in the SAME directory
//! (so the rename below is on one filesystem, which is what makes it atomic on POSIX) and is only
//! made visible by an atomic rename. A half-written cursor file is never observable as a cursor --
//! a reader either sees the old value or the new one, never a partial one.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::cursor::{CursorError, CursorStore};

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct FileCursorStore {
    dir: PathBuf,
}

impl FileCursorStore {
    /// `dir` need not exist yet -- `save` creates it (and any missing parents) on first write.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn cursor_path(&self, sink_id: &str) -> PathBuf {
        self.dir.join(format!("{sink_id}.cursor"))
    }
}

impl CursorStore for FileCursorStore {
    /// `Ok(None)` for a missing file -- a fresh sink that has never saved a cursor, a legitimate
    /// first-run state. A file that exists but does not parse as a `u64` is a typed error: that
    /// distinction (absent vs. corrupt) is the entire point of this store.
    fn load(&self, sink_id: &'static str) -> Result<Option<u64>, CursorError> {
        let path = self.cursor_path(sink_id);
        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(fault(sink_id, &path, "read", &source)),
        };
        text.trim().parse::<u64>().map(Some).map_err(|e| CursorError {
            sink_id,
            reason: format!("cursor file {} is corrupt: {e}", path.display()),
        })
    }

    fn save(&self, sink_id: &'static str, seq: u64) -> Result<(), CursorError> {
        fs::create_dir_all(&self.dir)
            .map_err(|source| fault(sink_id, &self.dir, "create directory", &source))?;
        let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let tmp = self.dir.join(format!("{sink_id}.cursor.tmp-{}-{n}", std::process::id()));
        fs::write(&tmp, seq.to_string()).map_err(|source| fault(sink_id, &tmp, "write", &source))?;
        let dest = self.cursor_path(sink_id);
        fs::rename(&tmp, &dest).map_err(|source| fault(sink_id, &dest, "rename into place", &source))
    }
}

fn fault(sink_id: &'static str, path: &Path, action: &str, source: &std::io::Error) -> CursorError {
    CursorError { sink_id, reason: format!("could not {action} {}: {source}", path.display()) }
}
