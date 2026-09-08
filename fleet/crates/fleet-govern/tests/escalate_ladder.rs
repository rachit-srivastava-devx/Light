//! Escalation ladder ordering and tie-breaking (§9's `escalate_ladder_returns_most_severe_
//! matching_rung`, `escalate_below_every_threshold_is_continue`, `escalate_above_pause_is_pause`).

use std::time::Duration;

use fleet_govern::{escalate, Escalation, EscalationPolicy};
use fleet_types::Tokens;

fn policy() -> EscalationPolicy {
    EscalationPolicy {
        throttle_at: Tokens::new(100),
        downgrade_at: Tokens::new(200),
        cached_at: Tokens::new(300),
        pause_at: Tokens::new(400),
    }
}

#[test]
fn below_every_threshold_is_continue() {
    let got = escalate(Tokens::new(50), &policy(), Duration::from_secs(1));
    assert_eq!(got, Escalation::Continue);
}

#[test]
fn above_pause_is_pause() {
    let got = escalate(Tokens::new(u64::MAX), &policy(), Duration::from_secs(1));
    assert_eq!(got, Escalation::Pause);
}

#[test]
fn each_rung_fires_at_its_own_threshold() {
    let p = policy();
    let delay = Duration::from_secs(5);
    assert_eq!(escalate(p.throttle_at, &p, delay), Escalation::Throttle { delay });
    assert_eq!(escalate(p.downgrade_at, &p, delay), Escalation::Downgrade);
    assert_eq!(escalate(p.cached_at, &p, delay), Escalation::UseCached);
    assert_eq!(escalate(p.pause_at, &p, delay), Escalation::Pause);
}

#[test]
fn ties_resolve_to_the_most_severe_matching_rung() {
    // Degenerate policy: every threshold equal. The highest rung (Pause) must win, not the
    // first match-arm accident (Throttle).
    let degenerate = EscalationPolicy {
        throttle_at: Tokens::new(100),
        downgrade_at: Tokens::new(100),
        cached_at: Tokens::new(100),
        pause_at: Tokens::new(100),
    };
    let got = escalate(Tokens::new(100), &degenerate, Duration::from_secs(1));
    assert_eq!(got, Escalation::Pause);
}
