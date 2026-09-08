//! (b) Resume after a simulated restart, no duplicated work: a fresh `AutonomousRun` built over a
//! fresh `FileLoopStore` handle to the *same on-disk file* must pick up exactly where a prior
//! process left off -- proving durability comes from the injected store port, not anything held
//! in memory across the "restart" (a new struct, new file handle, same path).

mod support;

use std::collections::BTreeSet;
use std::time::{Duration, SystemTime};

use fleet_govern::{AutonomousRun, FileLoopStore, MeterStore, TickOutcome, UnitId};
use fleet_types::Tokens;
use support::{lane, measured, plan, policy_never_pauses, FixedCooldown, InMemoryStore};

#[test]
fn resumes_after_a_restart_without_redoing_completed_units() {
    let dir = tempfile::tempdir().unwrap();
    let progress_path = dir.path().join("loop-progress.tsv");

    let meter = InMemoryStore::new();
    meter.seed(&lane("codex"), measured(10_000, 0));
    let cooldowns = FixedCooldown { cooling: BTreeSet::new() };
    let run_plan = plan("multi-day-demo", &["feature-a", "feature-b", "feature-c"], 100);
    let capable = || BTreeSet::from(["codex"]);
    let now = SystemTime::UNIX_EPOCH;

    // Day 1: a process runs one unit to completion, then exits (simulated by dropping `run`).
    {
        let store = FileLoopStore::new(progress_path.clone());
        let run = AutonomousRun {
            plan: run_plan.clone(),
            meter: &meter,
            cooldowns: &cooldowns,
            progress: &store,
            policy: policy_never_pauses(),
            retry_after: Duration::from_secs(60),
        };
        let TickOutcome::Advanced { unit, decision, reservation } = run.tick(now, capable()).unwrap() else {
            panic!("expected Advanced")
        };
        assert_eq!(unit, UnitId::new("feature-a"));
        assert_eq!(decision.selected_adapter, Some("codex"));
        run.complete_unit(&unit, reservation, Tokens::new(100)).unwrap();
    }

    // Day 3 ("restart"): a brand new `AutonomousRun` over a brand new `FileLoopStore` handle to
    // the same path must resume at unit 2, never re-offering unit 1.
    let later = now + Duration::from_secs(86_400 * 2);
    let store = FileLoopStore::new(progress_path.clone());
    let run = AutonomousRun {
        plan: run_plan,
        meter: &meter,
        cooldowns: &cooldowns,
        progress: &store,
        policy: policy_never_pauses(),
        retry_after: Duration::from_secs(60),
    };
    let TickOutcome::Advanced { unit, reservation, .. } = run.tick(later, capable()).unwrap() else {
        panic!("expected Advanced")
    };
    assert_eq!(unit, UnitId::new("feature-b"), "must resume at the next unit, not redo feature-a");
    run.complete_unit(&unit, reservation, Tokens::new(100)).unwrap();

    let TickOutcome::Advanced { unit, reservation, .. } = run.tick(later, capable()).unwrap() else {
        panic!("expected Advanced")
    };
    assert_eq!(unit, UnitId::new("feature-c"));
    run.complete_unit(&unit, reservation, Tokens::new(100)).unwrap();

    assert!(matches!(run.tick(later, capable()).unwrap(), TickOutcome::Done));

    // Exactly 3 admits of 100 each landed, never 4 -- no duplicated work from the restart.
    let remaining = meter.snapshot_remaining().unwrap();
    assert_eq!(remaining.get("codex").copied().flatten(), Some(Tokens::new(10_000 - 300)));
}
