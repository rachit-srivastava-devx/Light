//! The durable pipeline graph: `event -> classify -> scan -> plan -> dispatch -> verify ->
//! merge -> teach`. **Restate is deferred in this pass** (see `graph.rs`'s doc comment for why
//! and what changes to adopt it later) -- `step_log` provides the crash-resume property via a
//! resumable step log on disk instead of `restate_sdk`'s journal.

pub mod channels;
mod classify_stage;
pub mod ctx;
pub mod dispatch_table;
pub mod event;
mod event_ingest;
mod event_stage;
pub mod graph;
pub(crate) mod ledger_events;
mod ledger_log_source;
mod merge_stage;
pub mod planahead;
pub mod records;
mod run_ledger;
mod run_records;
pub mod stage;
mod stage_lld_nodes;
mod stage_loop;
pub mod stage_report;
pub mod stages;
pub mod stages_dispatch;
pub mod step_log;
mod stream_flush;
mod teach_stage;
mod verify_stage;

pub use graph::run_pipeline;
pub use stream_flush::maybe_flush;
