//! Implements owner asks #7/#8 (`blueprints/REQUIREMENTS.md`): "detect the module is good to go
//! and start working on it while streaming the next module details." Generalizes the
//! `channels::blueprint_q` -> `build_q` pair (previously a same-call round trip in
//! `stages_dispatch.rs`, never actually overlapped) to an arbitrary caller-defined sequence of
//! "units" (modules): a planner task fills a *bounded* tokio channel while a builder task drains
//! it, so build(N) and plan(N+1) run concurrently, bounded by the channel capacity (sized from
//! the same `ConcurrencyCap` every other lane uses) rather than by an unbounded queue racing
//! ahead of a slow builder. `UnitLog` gives the crash-resume property: a restart skips any unit
//! already `Planned`/`Built` instead of redoing it.

use super::error::PlanAheadError;
use super::unit_log::UnitLog;
use super::workers::{build_units, plan_units};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

#[cfg(test)]
#[path = "test_support.rs"]
mod test_support;

#[cfg(test)]
#[path = "orchestrator_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "crash_resume_tests.rs"]
mod crash_resume_tests;

/// A unit's planning or build step: `&str` in, `Ok(())` or a plain error message out --
/// deliberately synchronous (real planning/building is CPU/subprocess-bound, not
/// IO-await-bound), so callers pass a plain closure and this module owns both where it runs
/// (`spawn_blocking`) and how its failure gets tagged (`PlanAheadError::Plan`/`Build`, which
/// phase and unit id it was).
pub type UnitStep = Arc<dyn Fn(&str) -> Result<(), String> + Send + Sync>;

/// Runs `units` through plan-then-build with the next unit's planning overlapping the current
/// unit's build. `queue_capacity` (pass `ConcurrencyCap::get()`) is the backpressure bound: the
/// planner blocks on `send` once that many planned-but-not-yet-built units are in flight, so a
/// slow builder halts run-ahead instead of the queue growing without limit.
pub async fn run_plan_ahead(
    units: Vec<String>,
    state_dir: PathBuf,
    run_id: &str,
    queue_capacity: usize,
    plan_fn: UnitStep,
    build_fn: UnitStep,
) -> Result<(), PlanAheadError> {
    let log = Arc::new(UnitLog::open(&state_dir, run_id));
    let (tx, rx) = mpsc::channel::<String>(queue_capacity.max(1));

    let planner = tokio::spawn(plan_units(units, Arc::clone(&log), tx, plan_fn));
    let builder = tokio::spawn(build_units(Arc::clone(&log), rx, build_fn));

    let plan_result = planner.await.map_err(|e| PlanAheadError::WorkerPanicked(e.to_string()))?;
    let build_result = builder.await.map_err(|e| PlanAheadError::WorkerPanicked(e.to_string()))?;
    plan_result?;
    build_result
}
