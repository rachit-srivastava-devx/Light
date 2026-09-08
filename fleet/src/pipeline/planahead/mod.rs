//! Plan-ahead build overlap (owner asks #7/#8, `blueprints/REQUIREMENTS.md`): while unit N is
//! being built, unit N+1 is already being planned, bounded by a backpressured queue and resumable
//! across a crash. See `orchestrator.rs` for the mechanism and `unit_log.rs` for the durability
//! property; both are split out to stay under this tree's 80-line-per-file gate.

mod error;
mod orchestrator;
mod unit_log;
mod workers;

pub use error::PlanAheadError;
pub use orchestrator::{run_plan_ahead, UnitStep};
