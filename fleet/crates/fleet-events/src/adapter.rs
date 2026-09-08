//! `Adapter` -- one pull-based ingress source, open/closed.

use crate::clock::Clock;
use crate::envelope::EventEnvelope;
use crate::source_kind::SourceKind;

/// One ingress source. Every concrete adapter implements this and nothing else touches the
/// trait -- adding a fifth source is a fifth file behind this trait.
pub trait Adapter {
    type Error: std::error::Error + Send + Sync + 'static;

    fn source(&self) -> SourceKind;

    /// Pull whatever is newly available since this adapter's own last call. Never blocks
    /// indefinitely. Returns an empty `Vec` -- not an error -- when nothing new is available;
    /// only a genuine IO/protocol failure is `Err`.
    fn pull(&mut self, clock: &dyn Clock) -> Result<Vec<EventEnvelope>, Self::Error>;
}
