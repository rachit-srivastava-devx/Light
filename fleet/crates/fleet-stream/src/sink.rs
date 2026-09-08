//! One delivery target. Implementations must never block the pump indefinitely inside
//! `accepts` (a pure, fast predicate) and must treat `deliver` as the one place real IO happens.

use crate::event::StreamEvent;

pub trait Sink: Send {
    /// Stable identity: the `CursorStore` key and every log line/metric about this sink names
    /// it. Never renamed once shipped -- renaming silently resets that sink's resume position.
    fn id(&self) -> &'static str;
    /// Whether this sink wants to see `event` at all. Checked before `event` is ever queued.
    fn accepts(&self, event: &StreamEvent) -> bool;
    /// Attempt one delivery. `&mut self` because a sink typically owns a live connection/handle
    /// that delivery mutates or writes through.
    fn deliver(&mut self, event: &StreamEvent) -> Result<(), SinkError>;
}

/// Why a delivery attempt failed, and whether `pump` should retry it.
#[derive(Debug, thiserror::Error)]
pub enum SinkError {
    /// Retry-worthy: the sink (or the network to it) is temporarily unavailable.
    #[error("sink {sink} temporarily unavailable: {reason}")]
    Transient { sink: &'static str, reason: String },
    /// Not retry-worthy: the sink has permanently rejected this specific event.
    #[error("sink {sink} permanently rejected event seq {seq}: {reason}")]
    Permanent {
        sink: &'static str,
        seq: u64,
        reason: String,
    },
}
