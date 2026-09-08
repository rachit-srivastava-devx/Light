//! `settle` -- closes out a reservation, replacing its estimate with the real measured count.
//! `used` moves by `checked_sub(estimated)` then `checked_add(actual)`, never re-derived from
//! scratch, so a concurrent lane's own usage is never clobbered.

use fleet_types::Tokens;

use crate::store::MeterStore;
use crate::types::{LaneState, Reservation, SettleError};

pub fn settle(store: &dyn MeterStore, reservation: Reservation, actual: Tokens) -> Result<(), SettleError> {
    let mut outcome: Option<Result<(), SettleError>> = None;
    store.with_lane_locked(&reservation.lane, &mut |slot| {
        outcome = Some(settle_locked(&reservation, actual, slot));
    })?;
    outcome.expect("with_lane_locked always invokes f exactly once")
}

fn settle_locked(
    reservation: &Reservation,
    actual: Tokens,
    slot: &mut Option<LaneState>,
) -> Result<(), SettleError> {
    let state = slot
        .as_mut()
        .ok_or(SettleError::UnknownReservation(reservation.id))?;
    let pos = state
        .reservations
        .iter()
        .position(|r| r.id == reservation.id)
        .ok_or(SettleError::UnknownReservation(reservation.id))?;
    let held = state.reservations.remove(pos);
    let used = state.used.unwrap_or(Tokens::ZERO);
    let used = used.checked_sub(held.estimated)?;
    let used = used.checked_add(actual)?;
    state.used = Some(used);
    Ok(())
}
