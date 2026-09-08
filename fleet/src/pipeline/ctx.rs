//! `StageCtx`: everything a stage function needs from its caller, gathered in one place instead
//! of each stage reaching into ambient state (env vars, `std::env::current_dir`, ...). Read-only,
//! borrowed -- the pipeline graph owns everything named here.

use fleet_types::TaskId;
use std::path::Path;

pub struct StageCtx<'a> {
    /// Where `StepLog` and the ledger live for this run.
    pub state_dir: &'a Path,
    /// The repo `Merge` inspects for real staged/branch state.
    pub repo: &'a Path,
    pub task: &'a TaskId,
    pub runtime: &'a fleet_router::RuntimeState,
    /// The gate table `Verify` runs. Callers that risk re-entering their own `cargo test` (the
    /// hidden `__pipeline_probe`, invoked BY an integration test that a `cargo test --workspace`
    /// gate would itself re-trigger) pass a filtered table; `fleet run` passes the real, full
    /// `fleet_verify::GATES`.
    pub verify_gates: &'a [fleet_verify::GateSpec],
}
