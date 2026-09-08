//! `ingest_once` -- the one spawn-on-demand ingest cycle.

use crate::adapter::Adapter;
use crate::clock::Clock;
use crate::guard::{guard, GuardedAction};
use crate::sink::{EventSink, SinkError};

/// Why one `ingest_once` call failed.
#[derive(Debug, thiserror::Error)]
pub enum IngestError<E: std::error::Error + 'static> {
    #[error("adapter pull failed: {0}")]
    Pull(#[source] E),
    #[error("event sink rejected an envelope: {0}")]
    Sink(#[source] SinkError),
}

/// What one `ingest_once` call did, for the caller to log/print.
#[derive(Debug, Default)]
pub struct IngestReport {
    pub pulled: usize,
    pub written: usize,
    pub guarded: Vec<GuardedAction>,
}

/// Run exactly one spawn-on-demand ingest cycle: call `adapter.pull` once, run `guard` over every
/// returned envelope, write each through `sink`, and return. Never loops, never re-polls.
///
/// On a sink failure partway through the batch: envelopes already written stay written; this fn
/// returns `Err` immediately without attempting the rest, and no `IngestReport` is returned --
/// the caller retries the whole cycle (safe, since `EventId` derivation and `EventSink::append`
/// are both idempotent).
pub fn ingest_once<A: Adapter, S: EventSink>(
    adapter: &mut A,
    sink: &S,
    clock: &dyn Clock,
) -> Result<IngestReport, IngestError<A::Error>> {
    let envelopes = adapter.pull(clock).map_err(IngestError::Pull)?;
    let mut report = IngestReport { pulled: envelopes.len(), written: 0, guarded: Vec::new() };
    for envelope in &envelopes {
        report.guarded.push(guard(envelope));
        sink.append(envelope).map_err(IngestError::Sink)?;
        report.written += 1;
    }
    Ok(report)
}
