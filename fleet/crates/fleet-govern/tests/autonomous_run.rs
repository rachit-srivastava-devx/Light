//! (a) `AutonomousRun::tick` switches providers once the current top-preference lane's quota is
//! spent. The "no busy loop on full exhaustion" case (c) lives in `autonomous_run_pause.rs`, and
//! the restart/resume case (b) in `autonomous_run_restart.rs` -- split three ways purely to hold
//! the 80-line-per-file rule, not because the scenarios are unrelated.

mod support;

use std::collections::BTreeSet;
use std::time::{Duration, SystemTime};

use fleet_govern::{AutonomousRun, TickOutcome};
use fleet_types::Tokens;
use support::{lane, measured, plan, policy_never_pauses, FixedCooldown, InMemoryLoopStore, InMemoryStore};

#[test]
fn tick_switches_provider_when_top_preference_lane_is_exhausted() {
    let meter = InMemoryStore::new();
    meter.seed(&lane("codex"), measured(100, 0)); // exactly one unit's worth, then spent
    meter.seed(&lane("claude"), measured(1000, 0));
    let cooldowns = FixedCooldown { cooling: BTreeSet::new() };
    let progress = InMemoryLoopStore::new();
    let run = AutonomousRun {
        plan: plan("demo", &["feature-a", "feature-b"], 100),
        meter: &meter,
        cooldowns: &cooldowns,
        progress: &progress,
        policy: policy_never_pauses(),
        retry_after: Duration::from_secs(3600),
    };
    let capable = || BTreeSet::from(["codex", "claude"]);
    let now = SystemTime::UNIX_EPOCH;

    let first = run.tick(now, capable()).unwrap();
    let TickOutcome::Advanced { unit, decision, reservation } = first else { panic!("expected Advanced") };
    assert_eq!(decision.selected_adapter, Some("codex"));
    run.complete_unit(&unit, reservation, Tokens::new(100)).unwrap();

    // codex's 100-token window is now fully spent -- the second unit must switch to claude.
    let second = run.tick(now, capable()).unwrap();
    let TickOutcome::Advanced { unit, decision, reservation } = second else { panic!("expected Advanced") };
    assert_eq!(decision.selected_adapter, Some("claude"));
    run.complete_unit(&unit, reservation, Tokens::new(50)).unwrap();

    assert!(matches!(run.tick(now, capable()).unwrap(), TickOutcome::Done));
}
