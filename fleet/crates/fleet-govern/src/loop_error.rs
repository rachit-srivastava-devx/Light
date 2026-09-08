//! `LoopError` -- the single typed error `AutonomousRun::tick`/`complete_unit` return, composing
//! every fallible port this crate's loop driver touches. Never a `String`/`bool`/`Option` stand-in.

use crate::loop_types::LoopIoError;
use crate::types::{AdmitError, MeterIoError, SettleError};
use fleet_types::EmptyIdentifier;

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum LoopError {
    #[error("loop progress store IO failed: {0}")]
    Store(#[from] LoopIoError),
    #[error("meter store IO failed: {0}")]
    Meter(#[from] MeterIoError),
    #[error(transparent)]
    Admit(#[from] AdmitError),
    #[error(transparent)]
    Settle(#[from] SettleError),
    #[error("router selected an adapter name fleet-types rejected: {0}")]
    BadAdapter(#[from] EmptyIdentifier),
    #[error("complete_unit called for {0:?} but tick's last Advanced outcome named a different unit")]
    UnitMismatch(String),
}
