//! Validate, redact, and normalize external events before durable storage.

pub mod dedup;
pub mod types;

mod api;
mod attachment;
mod connector;
mod durable;
mod injection;
mod normalize;
mod redact;
mod registry;

pub use api::ingest;
pub use connector::normalize_connector;
pub use dedup::{InboxDecision, SeenIds};
pub use durable::{CommittedEventRef, ControlEventRef, DurableInbox, SqlDurableInbox};
pub use normalize::{normalize, normalize_authenticated};
pub use types::*;
