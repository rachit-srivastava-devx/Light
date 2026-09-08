//! `fleet __planahead_probe`: parse -> `pipeline::planahead::run_plan_ahead` -> print. The one
//! real call site that makes `pipeline::planahead` reachable from the compiled binary, matching
//! `run_cmd::pipeline_probe`'s role for `pipeline::graph`.

use crate::cli::args_ctx::PlanAheadProbeArgs;
use crate::dispatch::error::DispatchError;
use crate::pipeline::planahead::{run_plan_ahead, UnitStep};
use crate::print::human;
use std::path::Path;
use std::sync::Arc;

/// A no-op step: this probe exists to prove the overlap/backpressure/resume *mechanism* wired
/// end to end through the real binary (`src/tests/planahead_probe_*.rs`), not to run real
/// planning/build work -- exactly how `__pipeline_probe` uses `NO_GATES` to stay fast and
/// deterministic rather than shelling out to real tools.
fn noop_step() -> UnitStep {
    Arc::new(|_unit| Ok(()))
}

pub fn probe(state_dir: &Path, args: PlanAheadProbeArgs) -> Result<(), DispatchError> {
    let rt = tokio::runtime::Handle::current();
    let result = tokio::task::block_in_place(|| {
        rt.block_on(run_plan_ahead(
            args.units.clone(),
            state_dir.to_path_buf(),
            &args.run_id,
            args.queue_capacity,
            noop_step(),
            noop_step(),
        ))
    });
    result.map_err(DispatchError::PlanAhead)?;
    human::line("planahead", format!("{} unit(s) planned+built", args.units.len()));
    Ok(())
}
