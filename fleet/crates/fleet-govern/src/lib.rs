//! Budget admission, token metering, and quota-aware failover.
//!
//! This crate answers three questions with zero ambient IO: "may this lane spend N more tokens
//! right now, and if so, hold them" (`admit`/`settle`), "given what's currently capable, quota'd,
//! and cooling down, which candidate should `fleet-router` pick" (`next_provider`), and "spend is
//! approaching budget -- what should the caller do about it" (`escalate`). Every fact about the
//! outside world (the meter file, the wall clock, which adapters are installed) arrives through
//! an injected port; this crate never calls `SystemTime::now()`, never spawns a process, and
//! never reads an environment variable inside its own logic.

mod admit;
mod cooldown_file;
mod escalate;
mod failover;
mod file_lock;
mod loop_complete;
mod loop_error;
mod loop_outcome;
mod loop_row_codec;
mod loop_run;
mod loop_store;
mod loop_store_file;
mod loop_types;
mod meter_codec;
mod reservation_codec;
mod row_codec;
mod settle;
mod store;
mod store_file;
mod tokenizer;
mod types;

pub use admit::admit;
pub use cooldown_file::FileCooldownStore;
pub use escalate::{escalate, Escalation, EscalationPolicy};
pub use failover::{next_provider, FailoverInputs};
pub use loop_error::LoopError;
pub use loop_outcome::TickOutcome;
pub use loop_run::AutonomousRun;
pub use loop_store::LoopStore;
pub use loop_store_file::FileLoopStore;
pub use loop_types::{LoopIoError, LoopPlan, LoopProgress, UnitId};
pub use settle::settle;
pub use store::{CooldownStore, MeterStore};
pub use store_file::FileMeterStore;
pub use tokenizer::{estimate, TiktokenTokenizer, Tokenizer, TokenizerLoadError};
pub use types::{AdmitError, LaneState, MeterIoError, Reservation, ReservationId, SettleError};
