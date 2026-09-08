//! Typed failure modes for resolving/materializing the freelane asset. Mirrors
//! `crates/fleet-verify/src/gates/error.rs::GateAssetError`.

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum FreelaneAssetError {
    /// A caller-supplied `$FLEET_FREELANE_ROOT` override does not exist (or is not a directory).
    #[error("freelane-root override does not exist or is not a directory: {0}")]
    OverrideMissing(PathBuf),

    /// The embedded asset could not be written out to a fresh temp directory.
    #[error("failed to materialize embedded freelane asset: {0}")]
    Materialize(#[source] std::io::Error),

    /// `freelane.sh` is not present under the resolved root (embedded or overridden).
    #[error("freelane.sh not found under the freelane root: {0}")]
    ScriptMissing(PathBuf),
}
