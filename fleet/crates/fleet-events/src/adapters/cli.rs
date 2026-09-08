//! `CliAdapter` -- wraps a single pre-read argv/stdin invocation. No IO of its own: the
//! composition root already performed the OS-level read before construction.

use crate::adapter::Adapter;
use crate::clock::Clock;
use crate::envelope::EventEnvelope;
use crate::event_kind::EventKind;
use crate::source_kind::SourceKind;
use crate::ids::EventId;
use serde_json::json;

/// This adapter can never fail -- its input was already read by the caller.
#[derive(Debug, thiserror::Error)]
pub enum CliAdapterError {}

/// Yields exactly one envelope (built from the argv/stdin it was constructed with) on the first
/// `pull`, then `Ok(vec![])` on every subsequent call.
pub struct CliAdapter {
    args: Vec<String>,
    stdin: String,
    nonce: String,
    emitted: bool,
}

impl CliAdapter {
    pub fn new(args: Vec<String>, stdin: String, nonce: String) -> Self {
        Self { args, stdin, nonce, emitted: false }
    }
}

impl Adapter for CliAdapter {
    type Error = CliAdapterError;

    fn source(&self) -> SourceKind {
        SourceKind::Cli
    }

    fn pull(&mut self, clock: &dyn Clock) -> Result<Vec<EventEnvelope>, Self::Error> {
        if self.emitted {
            return Ok(vec![]);
        }
        self.emitted = true;
        let id = EventId::derive(SourceKind::Cli, EventKind::CliInvoked, &self.nonce);
        let payload = json!({ "args": self.args, "stdin": self.stdin });
        let envelope = EventEnvelope::new(
            id,
            SourceKind::Cli,
            EventKind::CliInvoked,
            clock.now_rfc3339(),
            payload,
        );
        Ok(vec![envelope])
    }
}
