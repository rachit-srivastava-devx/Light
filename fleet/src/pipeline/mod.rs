//! The durable pipeline graph: `event -> classify -> scan -> plan -> dispatch -> verify ->
//! merge -> teach`. **Restate is deferred in this pass** (see `graph.rs`'s doc comment for why
//! and what changes to adopt it later) -- `step_log` provides the crash-resume property via a
//! resumable step log on disk instead of `restate_sdk`'s journal.

pub mod channels;
pub mod dispatch_table;
pub mod event;
pub mod graph;
pub mod stage;
pub mod stages;
pub mod stages_dispatch;
pub mod step_log;

pub use graph::run_pipeline;
