//! Typed failure modes for resolving/materializing gate script assets.

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum GateAssetError {
    /// A caller-supplied gates-root override does not exist (or is not a directory).
    #[error("gates-root override does not exist or is not a directory: {0}")]
    OverrideMissing(PathBuf),

    /// Embedded assets could not be written out to a fresh temp directory.
    #[error("failed to materialize embedded gate assets: {0}")]
    Materialize(#[source] std::io::Error),

    /// A `GateCommand::Script`'s `relative` path is not present under the resolved gates root
    /// (embedded or overridden) -- distinct from `Materialize`/`OverrideMissing`, which are
    /// whole-root problems: this is one gate's asset missing from an otherwise-good root.
    #[error("unknown or missing gate script under the gates root: {0}")]
    UnknownScript(&'static str),
}
