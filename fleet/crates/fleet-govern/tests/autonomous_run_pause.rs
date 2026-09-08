//! (c) Full exhaustion across every capable lane returns a typed `Paused { until }` on the very
//! first call -- no busy loop, no internal retry, no sleep attempted inside this crate. See
//! `autonomous_run.rs`'s header for why this scenario lives in its own file.

mod support;

use std::collections::BTreeSet;
use std::time::{Duration, SystemTime};

use fleet_govern::{AutonomousRun, TickOutcome};
use support::{lane, measured, plan, policy_never_pauses, FixedCooldown, InMemoryLoopStore, InMemoryStore};

#[test]
fn tick_pauses_with_a_retry_time_when_every_lane_is_exhausted() {
    let meter = InMemoryStore::new();
    meter.seed(&lane("codex"), measured(50, 50));
    meter.seed(&lane("claude"), measured(50, 50));
    let cooldowns = FixedCooldown { cooling: BTreeSet::new() };
    let progress = InMemoryLoopStore::new();
    let retry_after = Duration::from_secs(900);
    let run = AutonomousRun {
        plan: plan("demo", &["feature-a"], 100),
        meter: &meter,
        cooldowns: &cooldowns,
        progress: &progress,
        policy: policy_never_pauses(),
        retry_after,
    };
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(86_400 * 3); // "days in", not day 0

    let outcome = run.tick(now, BTreeSet::from(["codex", "claude"])).unwrap();
    match outcome {
        TickOutcome::Paused { until } => assert_eq!(until, now + retry_after),
        other => panic!("expected Paused, got {other:?}"),
    }
}
