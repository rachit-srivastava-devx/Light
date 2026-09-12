//! `FreelaneRoot` -- where the freelane keyless-lane script is resolved from. Mirrors
//! `crates/fleet-verify/src/gates/root.rs::GatesRoot`.

use std::path::{Path, PathBuf};

use tempfile::TempDir;

use super::error::FreelaneAssetError;
use super::materialize::materialize;

/// Either a temp directory freshly populated from the embedded asset (kept alive for as long as
/// this value lives -- dropping it deletes the directory), or a caller-supplied directory on disk
/// used as-is in place of the embedded copy.
pub enum FreelaneRoot {
    Embedded(TempDir),
    Override(PathBuf),
}

impl FreelaneRoot {
    /// Materialize the embedded `freelane.sh` (+ `lanes.conf`) into a fresh temp directory.
    pub fn materialize() -> Result<Self, FreelaneAssetError> {
        let dir = tempfile::tempdir().map_err(FreelaneAssetError::Materialize)?;
        materialize(dir.path()).map_err(FreelaneAssetError::Materialize)?;
        Ok(Self::Embedded(dir))
    }

    /// Use a real directory on disk (laid out with `freelane.sh` directly inside it) instead of
    /// the embedded copy.
    pub fn from_override(path: impl Into<PathBuf>) -> Result<Self, FreelaneAssetError> {
        let path = path.into();
        if !path.is_dir() {
            return Err(FreelaneAssetError::OverrideMissing(path));
        }
        Ok(Self::Override(path))
    }

    fn path(&self) -> &Path {
        match self {
            Self::Embedded(dir) => dir.path(),
            Self::Override(path) => path,
        }
    }

    /// The `freelane.sh` script path inside this root, verified to actually exist there.
    pub fn script(&self) -> Result<PathBuf, FreelaneAssetError> {
        let resolved = self.path().join("freelane.sh");
        if resolved.is_file() {
            Ok(resolved)
        } else {
            Err(FreelaneAssetError::ScriptMissing(resolved))
        }
    }
}
