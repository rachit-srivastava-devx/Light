//! The throttle -> downgrade -> cached -> pause escalation ladder. Genuinely new (no fleet
//! source names it); pure, total, integer `Tokens` thresholds -- never a derived float ratio.

use std::time::Duration;

use fleet_types::Tokens;

/// Ascending token-count thresholds on a lane's *window* at which the caller should take the
/// next, more drastic action. Compared `>=`, highest first, so a degenerate policy with equal
/// thresholds resolves to the single most severe matching rung.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EscalationPolicy {
    pub throttle_at: Tokens,
    pub downgrade_at: Tokens,
    pub cached_at: Tokens,
    pub pause_at: Tokens,
}

/// What the caller should do next, in ascending severity. `escalate` itself never acts -- it has
/// no subprocess, cache, or sleep of its own.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Escalation {
    Continue,
    Throttle { delay: Duration },
    Downgrade,
    UseCached,
    Pause,
}

/// Compare `used` against `policy`'s thresholds and return the single most severe action that
/// applies. `delay` is a fixed, policy-supplied backoff, never computed from a clock read here.
pub fn escalate(used: Tokens, policy: &EscalationPolicy, throttle_delay: Duration) -> Escalation {
    if used >= policy.pause_at {
        Escalation::Pause
    } else if used >= policy.cached_at {
        Escalation::UseCached
    } else if used >= policy.downgrade_at {
        Escalation::Downgrade
    } else if used >= policy.throttle_at {
        Escalation::Throttle { delay: throttle_delay }
    } else {
        Escalation::Continue
    }
}
