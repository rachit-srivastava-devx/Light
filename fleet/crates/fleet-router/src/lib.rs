//! Deterministic, side-effect-free role -> model routing decision.
//!
//! This crate answers exactly one question -- "given a role, a task class, and a snapshot of
//! what is currently available, which adapter/model gets this task, or why not" -- and answers it
//! identically every time for the same inputs. It performs no IO, spawns no process, reads no
//! clock, and reads no environment variable: every fact about the world arrives through
//! `RuntimeState`, which the caller builds from its own probes.

mod allow;
mod decide;
mod pick;
mod role_check;
mod stage;
mod table;
mod types;
mod verify_gate;

pub use decide::decide;
pub use role_check::{evaluate_role_check, RoleCheck, RoleRefusal};
pub use table::{CandidateSpec, TaskClass, Tier, ORDER};
pub use types::{Decision, Refusal, RuntimeState, Stage};
