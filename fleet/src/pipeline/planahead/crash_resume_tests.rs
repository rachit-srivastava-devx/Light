//! (c): a restart mid-run resumes without duplicating planned-or-built work. Runs the real
//! `UnitLog` on disk across two separate `run_plan_ahead` calls sharing the same `state_dir`/
//! `run_id`, counting how many times each unit is actually planned/built across both.

use super::run_plan_ahead;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

type Counts = Arc<Mutex<HashMap<String, u32>>>;

fn counting_step(counts: Counts) -> super::UnitStep {
    Arc::new(move |unit| {
        *counts.lock().unwrap_or_else(|p| p.into_inner()).entry(unit.to_string()).or_insert(0) += 1;
        Ok(())
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn a_restart_never_replans_or_rebuilds_an_already_finished_unit() {
    let dir = tempfile::tempdir().unwrap();
    let plan_counts: Counts = Arc::default();
    let build_counts: Counts = Arc::default();

    // "Run 1": only units 1 and 2 exist yet -- stands in for the process that built them, then
    // the run stopping (a real crash would look identical from `UnitLog`'s point of view: some
    // units marked Built on disk, nothing else).
    let first = vec!["1".to_string(), "2".to_string()];
    run_plan_ahead(first, dir.path().to_path_buf(), "resume-run", 1, counting_step(plan_counts.clone()), counting_step(build_counts.clone()))
        .await
        .unwrap();

    // "Run 2": a fresh call (new channels, new tokio tasks -- the same shape a restarted process
    // would take) against the SAME state_dir/run_id, now with two more units to do.
    let second = vec!["1".to_string(), "2".to_string(), "3".to_string(), "4".to_string()];
    run_plan_ahead(second, dir.path().to_path_buf(), "resume-run", 1, counting_step(plan_counts.clone()), counting_step(build_counts.clone()))
        .await
        .unwrap();

    let plans = plan_counts.lock().unwrap();
    let builds = build_counts.lock().unwrap();
    for unit in ["1", "2", "3", "4"] {
        assert_eq!(plans.get(unit).copied().unwrap_or(0), 1, "unit {unit} was planned {:?} times", plans.get(unit));
        assert_eq!(builds.get(unit).copied().unwrap_or(0), 1, "unit {unit} was built {:?} times", builds.get(unit));
    }
}
