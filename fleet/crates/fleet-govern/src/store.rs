//! The injected persistence ports -- `MeterStore` (the atomic admit/settle boundary) and
//! `CooldownStore` (the clock-injected failure-backoff table).

use std::collections::BTreeMap;
use std::time::{Duration, SystemTime};

use fleet_types::{LaneId, Tokens};

use crate::types::{LaneState, MeterIoError};

/// The sole IO boundary this crate touches for the ledger. `with_lane_locked` is the entire
/// atomicity contract: the implementation must hold an exclusive lock across the
/// read-check-mutate-publish sequence so two concurrent callers can never both observe the same
/// `remaining` and both admit past it.
///
/// `f` returns `()`, not a generic `T` (the blueprint's literal `with_lane_locked<T>` signature
/// is not `dyn`-compatible in Rust -- a trait object cannot vtable a generic method, and every
/// caller of this port needs `&dyn MeterStore`). A caller that needs `f`'s result captures it
/// into a variable outside the closure instead (see `admit.rs`/`settle.rs`).
pub trait MeterStore: Send + Sync {
    /// Run `f` with exclusive access to `lane`'s current `LaneState` (absent lanes get `None`,
    /// never fabricated as configured-but-empty), then publish whatever `f` leaves behind
    /// durably before returning.
    fn with_lane_locked(
        &self,
        lane: &LaneId,
        f: &mut dyn FnMut(&mut Option<LaneState>),
    ) -> Result<(), MeterIoError>;

    /// A snapshot read of every configured lane's `remaining = window - used`, used only to
    /// build `RuntimeState.remaining` for `next_provider` -- never used to decide admission.
    fn snapshot_remaining(&self) -> Result<BTreeMap<String, Option<Tokens>>, MeterIoError>;
}

/// Tracks which adapters are in a measured failure-backoff window.
pub trait CooldownStore: Send + Sync {
    fn start(&self, adapter: &str, at: SystemTime, duration: Duration) -> Result<(), MeterIoError>;
    fn is_cooling_down(&self, adapter: &str, now: SystemTime) -> Result<bool, MeterIoError>;
}
