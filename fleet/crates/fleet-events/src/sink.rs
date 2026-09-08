//! `EventSink` -- the durable-write port this crate needs and does not implement.

use crate::envelope::EventEnvelope;

/// A failure appending to the durable event log.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum SinkError {
    #[error("event log unavailable: {0}")]
    Unavailable(String),
}

/// The durable-write port this crate needs and does not implement. `fleet-store` (once its own
/// blueprint exists) is expected to provide a type implementing this trait, or the composition
/// root adapts fleet-store's real API into one.
pub trait EventSink {
    /// MUST be idempotent on `envelope.id`: calling twice with an envelope carrying the same `id`
    /// is a successful no-op, never a duplicate row and never an error.
    fn append(&self, envelope: &EventEnvelope) -> Result<(), SinkError>;
}
