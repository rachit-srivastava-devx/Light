//! The durable pipeline graph: `event -> classify -> scan -> plan -> dispatch -> verify ->
//! merge -> teach`. **Restate is deferred in this pass** (see `graph.rs`'s doc comment for why
//! and what changes to adopt it later) -- `step_log` provides the crash-resume property via a
//! resumable step log on disk instead of `restate_sdk`'s journal.

pub mod channels;
mod classify_stage;
pub mod ctx;
pub mod dispatch_table;
pub mod event;
mod event_stage;
pub mod graph;
mod merge_stage;
pub mod planahead;
pub mod stage;
pub mod stage_report;
pub mod stages;
pub mod stages_dispatch;
pub mod step_log;
mod teach_stage;
mod verify_stage;

pub use graph::run_pipeline;
