//! The planner and builder task bodies `run_plan_ahead` spawns, split out of `orchestrator.rs`
//! to stay under the 80-line file gate.

use super::error::PlanAheadError;
use super::orchestrator::UnitStep;
use super::unit_log::{UnitLog, UnitPhase};
use std::sync::Arc;
use tokio::sync::mpsc;

pub(super) async fn plan_units(
    units: Vec<String>,
    log: Arc<UnitLog>,
    tx: mpsc::Sender<String>,
    plan_fn: UnitStep,
) -> Result<(), PlanAheadError> {
    for unit in units {
        if log.is_done(&unit, UnitPhase::Built) {
            continue; // fully done in a prior run: neither re-plan nor re-enqueue it
        }
        if !log.is_done(&unit, UnitPhase::Planned) {
            run_step(&plan_fn, &unit, "plan").await?;
            log.mark_done(&unit, UnitPhase::Planned)?;
        }
        // Blocks here (backpressure) once `queue_capacity` planned units are awaiting build --
        // this is the overlap point: everything above already ran while the builder was busy.
        tx.send(unit).await.map_err(|_| PlanAheadError::BuildSideGone)?;
    }
    Ok(())
}

pub(super) async fn build_units(
    log: Arc<UnitLog>,
    mut rx: mpsc::Receiver<String>,
    build_fn: UnitStep,
) -> Result<(), PlanAheadError> {
    while let Some(unit) = rx.recv().await {
        if log.is_done(&unit, UnitPhase::Built) {
            continue; // resumed run: this unit was already built before the crash
        }
        run_step(&build_fn, &unit, "build").await?;
        log.mark_done(&unit, UnitPhase::Built)?;
    }
    Ok(())
}

async fn run_step(step: &UnitStep, unit: &str, phase: &'static str) -> Result<(), PlanAheadError> {
    let step = Arc::clone(step);
    let unit_owned = unit.to_string();
    let outcome = tokio::task::spawn_blocking(move || step(&unit_owned))
        .await
        .map_err(|e| PlanAheadError::WorkerPanicked(format!("{phase}: {e}")))?;
    outcome.map_err(|message| match phase {
        "plan" => PlanAheadError::Plan { unit: unit.to_string(), message },
        _ => PlanAheadError::Build { unit: unit.to_string(), message },
    })
}
