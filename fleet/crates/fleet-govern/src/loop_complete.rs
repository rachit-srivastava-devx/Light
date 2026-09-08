//! `AutonomousRun::complete_unit`/`lane_used` -- the other half of the `tick`/`complete_unit`
//! two-phase protocol (split from `loop_run.rs` purely to hold the 80-line file cap).

use fleet_types::{LaneId, Tokens};

use crate::loop_error::LoopError;
use crate::loop_run::AutonomousRun;
use crate::loop_types::UnitId;
use crate::settle::settle;
use crate::types::Reservation;

impl<'a> AutonomousRun<'a> {
    /// Close out the reservation `tick` handed back for `unit` and persist that `unit` as done.
    /// The settle-then-persist order matters: if `progress.save` fails, a retried `tick` sees the
    /// unit still pending and the caller must settle again on its next attempt -- progress is
    /// never marked done ahead of the budget actually being reconciled.
    pub fn complete_unit(&self, unit: &UnitId, reservation: Reservation, actual: Tokens) -> Result<(), LoopError> {
        let mut progress = self.progress.load(&self.plan.id)?.unwrap_or_default();
        let expected = self.plan.units.get(progress.completed.len());
        if expected != Some(unit) {
            return Err(LoopError::UnitMismatch(unit.as_str().to_string()));
        }
        settle(self.meter, reservation, actual)?;
        progress.completed.push(unit.clone());
        self.progress.save(&self.plan.id, &progress)?;
        Ok(())
    }

    pub(crate) fn lane_used(&self, lane: &LaneId) -> Result<Tokens, LoopError> {
        let mut used = None;
        self.meter.with_lane_locked(lane, &mut |slot| {
            used = slot.as_ref().and_then(|s| s.used);
        })?;
        Ok(used.unwrap_or(Tokens::ZERO))
    }
}
