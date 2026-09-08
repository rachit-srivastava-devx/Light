//! `TickOutcome` -- what one `AutonomousRun::tick` call decided. Never a bare `bool`/`Option`:
//! every non-progress outcome names why (no provider vs. escalation-ladder pause) and, for the
//! two "don't spin" cases, exactly when the caller may retry.

use std::time::SystemTime;

use fleet_router::Decision;

use crate::loop_types::UnitId;
use crate::types::Reservation;

/// The result of one `tick`. `Advanced` carries an already-`admit`-ted `Reservation` the caller
/// must eventually pass to `complete_unit` (or let expire/retry on a future tick if the work
/// fails) -- `tick` never marks a unit done itself, only `complete_unit` does, so a crash between
/// the two never loses or double-counts progress.
#[derive(Clone, Debug)]
pub enum TickOutcome {
    /// Budget reserved for `unit` on `decision`'s selected adapter/model; go do the work.
    /// `decision` is boxed -- `Decision` carries a `Vec<Stage>` audit trail (large and rarely
    /// needed by every call site) while every other variant is a few bytes, and clippy's
    /// `large_enum_variant` flags the size gap otherwise.
    Advanced { unit: UnitId, decision: Box<Decision>, reservation: Reservation },
    /// `next_provider` refused: every capable adapter is either cooling down or below the
    /// required quota right now. Not a busy-loop signal -- sleep until `until`, then retry.
    Paused { until: SystemTime },
    /// A provider was selected, but the escalation ladder (`escalate`) says its measured `used`
    /// has crossed `pause_at` -- the run pauses proactively rather than admitting anyway.
    Exhausted { until: SystemTime },
    /// Every unit in the plan is already complete.
    Done,
}
