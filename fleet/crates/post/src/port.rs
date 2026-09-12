//! Injected gate execution port — the only IO boundary for gate dispatch.
use std::path::Path;
use crate::{GateResult, GateSpec, PostError};

/// Injected boundary: execute one gate against the integrated repo tree.
///
/// Implementations must not mutate the repo; they may only observe it and
/// publish a counted result. A result with `checked == 0` is treated as
/// `ZeroCoverage` by the caller.
pub trait GateRunner {
    fn run(&self, gate: &GateSpec, repo: &Path) -> Result<GateResult, PostError>;
}
