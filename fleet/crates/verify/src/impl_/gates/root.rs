//! `GatesRoot` -- where `GateCommand::Script { relative, .. }` paths are resolved against.

use std::path::{Path, PathBuf};

use tempfile::TempDir;

use super::error::GateAssetError;
use super::materialize::materialize;

/// Either a temp directory freshly populated from the embedded assets (kept alive for as long as
/// this value lives -- dropping it deletes the directory), or a caller-supplied directory on disk
/// used as-is in place of the embedded copies.
pub enum GatesRoot {
    Embedded(TempDir),
    Override(PathBuf),
}

impl GatesRoot {
    /// Materialize the embedded gate assets into a fresh temp directory.
    pub fn materialize() -> Result<Self, GateAssetError> {
        let dir = tempfile::tempdir().map_err(GateAssetError::Materialize)?;
        materialize(dir.path()).map_err(GateAssetError::Materialize)?;
        Ok(Self::Embedded(dir))
    }

    /// Use real files on disk instead of the embedded copies. `path` must already exist and be a
    /// directory laid out the same way `gates/` is (the four named scripts directly inside it,
    /// plus `policy/` and `corpus/`).
    pub fn from_override(path: impl Into<PathBuf>) -> Result<Self, GateAssetError> {
        let path = path.into();
        if !path.is_dir() {
            return Err(GateAssetError::OverrideMissing(path));
        }
        Ok(Self::Override(path))
    }

    pub fn path(&self) -> &Path {
        match self {
            Self::Embedded(dir) => dir.path(),
            Self::Override(path) => path,
        }
    }

    /// Resolve one `GateCommand::Script`'s `relative` path against this root, failing with a
    /// typed error if it is not actually there (a stale override, a script this crate forgot to
    /// embed, etc.) rather than letting the caller hand a dangling path to `ProcessRunner`.
    pub fn require(&self, relative: &'static str) -> Result<PathBuf, GateAssetError> {
        let resolved = self.path().join(relative);
        if resolved.is_file() {
            Ok(resolved)
        } else {
            Err(GateAssetError::UnknownScript(relative))
        }
    }
}
