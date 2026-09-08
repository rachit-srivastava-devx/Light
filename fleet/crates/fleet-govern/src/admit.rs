//! `admit` -- the atomic admission fix. The arithmetic mirrors `meter.rs::reserve` exactly; the
//! atomicity comes entirely from running inside `MeterStore::with_lane_locked`'s exclusive lock,
//! which `meter.rs::save`'s unconditional `rename` never provided.

use fleet_types::{LaneId, Tokens};

use crate::store::MeterStore;
use crate::types::{AdmitError, LaneState, Reservation, ReservationId};

pub fn admit(store: &dyn MeterStore, lane: &LaneId, cost_est: Tokens) -> Result<Reservation, AdmitError> {
    let mut outcome: Option<Result<Reservation, AdmitError>> = None;
    store.with_lane_locked(lane, &mut |slot| {
        outcome = Some(admit_locked(lane, cost_est, slot));
    })?;
    outcome.expect("with_lane_locked always invokes f exactly once")
}

fn admit_locked(
    lane: &LaneId,
    cost_est: Tokens,
    slot: &mut Option<LaneState>,
) -> Result<Reservation, AdmitError> {
    let key = || lane.as_str().to_string();
    let state = slot.as_mut().ok_or_else(|| AdmitError::UnknownLane(key()))?;
    let window = state.window.ok_or_else(|| AdmitError::WindowUnknown(key()))?;
    let used = state.used.ok_or_else(|| AdmitError::UsedUnknown(key()))?;
    let remaining = window.checked_sub(used)?;
    if cost_est.get() > remaining.get() {
        return Err(AdmitError::InsufficientBudget { lane: key(), requested: cost_est, remaining });
    }
    let next_id = state.reservations.iter().map(|r| r.id.get()).max().unwrap_or(0) + 1;
    let reservation = Reservation { id: ReservationId::new(next_id), lane: lane.clone(), estimated: cost_est };
    state.used = Some(used.checked_add(cost_est)?);
    state.reservations.push(reservation.clone());
    Ok(reservation)
}
