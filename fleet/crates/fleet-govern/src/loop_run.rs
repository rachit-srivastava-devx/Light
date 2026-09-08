//! `AutonomousRun` -- the multi-day loop driver. Composes `next_provider` (switch providers on
//! quota exhaustion), `admit` (atomic budget), and `escalate` (the existing ladder) into a
//! `tick(now)` state machine; progress lives entirely behind the injected `LoopStore`, so a
//! restart days later re-`load`s it instead of trusting anything held in memory.
//! `complete_unit` (this struct's other half, closing the loop with `settle`) is in
//! `loop_complete.rs` -- split purely to hold the 80-line file cap, same `impl` target.

use std::collections::BTreeSet;
use std::time::{Duration, SystemTime};

use fleet_types::LaneId;

use crate::admit::admit;
use crate::escalate::{escalate, Escalation, EscalationPolicy};
use crate::failover::{next_provider, FailoverInputs};
use crate::loop_error::LoopError;
use crate::loop_outcome::TickOutcome;
use crate::loop_store::LoopStore;
use crate::loop_types::LoopPlan;
use crate::store::{CooldownStore, MeterStore};

pub struct AutonomousRun<'a> {
    pub plan: LoopPlan,
    pub meter: &'a dyn MeterStore,
    pub cooldowns: &'a dyn CooldownStore,
    pub progress: &'a dyn LoopStore,
    pub policy: EscalationPolicy,
    /// Fixed backoff `tick` reports as `until` when it pauses -- never computed from a clock read
    /// inside this crate, always `now + retry_after`.
    pub retry_after: Duration,
}

impl<'a> AutonomousRun<'a> {
    /// Advance the run by (at most) one unit. Idempotent to call repeatedly with the same `now`
    /// while no `complete_unit` has landed yet -- it re-reads persisted progress and re-decides
    /// every time, never caching a stale "current unit" in `self`.
    pub fn tick(&self, now: SystemTime, capable: BTreeSet<&'static str>) -> Result<TickOutcome, LoopError> {
        let progress = self.progress.load(&self.plan.id)?.unwrap_or_default();
        let Some(unit) = self.plan.units.get(progress.completed.len()) else {
            return Ok(TickOutcome::Done);
        };

        let inputs = FailoverInputs {
            role: self.plan.role,
            class: self.plan.class,
            builder_resolved_model: None,
            capable,
            required_tokens: self.plan.tokens_per_unit,
            now,
        };
        let decision = next_provider(self.meter, self.cooldowns, &inputs)?;
        let Some(adapter) = decision.selected_adapter else {
            return Ok(TickOutcome::Paused { until: now + self.retry_after });
        };

        let lane = LaneId::parse(adapter)?;
        let used = self.lane_used(&lane)?;
        if let Escalation::Pause = escalate(used, &self.policy, self.retry_after) {
            return Ok(TickOutcome::Exhausted { until: now + self.retry_after });
        }

        let reservation = admit(self.meter, &lane, self.plan.tokens_per_unit)?;
        Ok(TickOutcome::Advanced { unit: unit.clone(), decision: Box::new(decision), reservation })
    }
}
