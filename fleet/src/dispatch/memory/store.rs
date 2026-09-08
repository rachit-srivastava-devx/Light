//! `SowMemoryStore` -- JSON-file persistence for `fleet sow`'s memory rows, mirroring
//! `fleet-govern::FileMeterStore`'s read-or-empty / write-whole-file pattern (no daemon, no
//! lock manager: `sow` is a fast one-shot CLI command, not a long-running server).

use fleet_memory::MemoryItem;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
#[error("sow memory store io: {0}")]
pub struct MemoryStoreError(pub String);

pub struct SowMemoryStore {
    path: PathBuf,
}

impl SowMemoryStore {
    pub fn new(state_dir: &Path) -> Self {
        Self { path: state_dir.join("memory").join("sow.json") }
    }

    pub fn load(&self) -> Result<Vec<MemoryItem>, MemoryStoreError> {
        match fs::read_to_string(&self.path) {
            Ok(text) => serde_json::from_str(&text)
                .map_err(|e| MemoryStoreError(format!("decode {}: {e}", self.path.display()))),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(MemoryStoreError(format!("read {}: {e}", self.path.display()))),
        }
    }

    pub fn save(&self, items: &[MemoryItem]) -> Result<(), MemoryStoreError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| MemoryStoreError(format!("mkdir: {e}")))?;
        }
        let text = serde_json::to_string_pretty(items)
            .map_err(|e| MemoryStoreError(format!("encode: {e}")))?;
        fs::write(&self.path, text).map_err(|e| MemoryStoreError(format!("write {}: {e}", self.path.display())))
    }
}
