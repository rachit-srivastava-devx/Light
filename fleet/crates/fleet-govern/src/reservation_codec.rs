//! One `id:estimated` reservation field, split out of `meter_codec.rs` to hold the 80-line
//! rule.

use fleet_types::{LaneId, Tokens};

use crate::types::{MeterIoError, Reservation, ReservationId};

pub(crate) fn parse_reservation(lane: &str, entry: &str) -> Result<Reservation, MeterIoError> {
    let (id, est) = entry
        .split_once(':')
        .ok_or_else(|| MeterIoError(format!("bad reservation entry {entry:?}")))?;
    let id: u64 = id.parse().map_err(|e| MeterIoError(format!("bad reservation id: {e}")))?;
    let est: u64 = est.parse().map_err(|e| MeterIoError(format!("bad reservation est: {e}")))?;
    let lane = LaneId::parse(lane).map_err(|e| MeterIoError(format!("bad lane id: {e}")))?;
    Ok(Reservation { id: ReservationId::new(id), lane, estimated: Tokens::new(est) })
}
