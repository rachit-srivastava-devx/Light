//! Ledger state and typed errors: `LaneState`, `Reservation`, `ReservationId`, `AdmitError`,
//! `SettleError`, `MeterIoError`.

use fleet_types::{LaneId, Tokens, TokensOverflow};

/// One lane's measured state. `window`/`used` are `None` exactly when unmeasured -- never
/// coerced to zero.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct LaneState {
    pub window: Option<Tokens>,
    pub used: Option<Tokens>,
    pub reservations: Vec<Reservation>,
    pub resolved_model: Option<String>,
    pub unknown_observed: bool,
}

/// A held claim on a lane's budget between `admit` and `settle`. Unique per lane (assigned by
/// `admit` while the lane is locked), so `settle` can address exactly the reservation it closes.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ReservationId(u64);

impl ReservationId {
    pub(crate) fn new(value: u64) -> Self {
        Self(value)
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reservation {
    pub id: ReservationId,
    pub lane: LaneId,
    pub estimated: Tokens,
}

/// Why `admit` refused to reserve.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum AdmitError {
    #[error("lane {0:?} is not configured")]
    UnknownLane(String),
    #[error("lane {0:?} has no configured window (unmeasured, not unlimited)")]
    WindowUnknown(String),
    #[error("lane {0:?} usage is unmeasured (unknown_observed)")]
    UsedUnknown(String),
    #[error("lane {lane:?} would exceed its window: requested {requested:?} > remaining {remaining:?}")]
    InsufficientBudget { lane: String, requested: Tokens, remaining: Tokens },
    #[error("token arithmetic overflowed while reserving")]
    Overflow(#[from] TokensOverflow),
    #[error("meter store IO failed: {0}")]
    Store(#[from] MeterIoError),
}

/// Why `settle` could not close out a reservation.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SettleError {
    #[error("reservation {0:?} is not open on any known lane")]
    UnknownReservation(ReservationId),
    #[error("token arithmetic overflowed while settling")]
    Overflow(#[from] TokensOverflow),
    #[error("meter store IO failed: {0}")]
    Store(#[from] MeterIoError),
}

/// The meter file could not be read, decoded, or durably published.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("{0}")]
pub struct MeterIoError(pub String);
