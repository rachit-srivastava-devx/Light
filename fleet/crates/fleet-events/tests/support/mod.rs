//! Shared fakes for the ingest-orchestration test files.

use fleet_events::{Adapter, Clock, EventEnvelope, EventId, EventSink, SinkError, SourceKind};
use std::cell::RefCell;
use std::error::Error;
use std::fmt;

pub struct FixedClock;
impl Clock for FixedClock {
    fn now_rfc3339(&self) -> String {
        "2026-01-01T00:00:00Z".to_string()
    }
}

#[derive(Debug)]
pub struct FakeAdapterError;
impl fmt::Display for FakeAdapterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "fake adapter error")
    }
}
impl Error for FakeAdapterError {}

pub struct FakeAdapter {
    pub batch: Vec<EventEnvelope>,
}
impl Adapter for FakeAdapter {
    type Error = FakeAdapterError;
    fn source(&self) -> SourceKind {
        SourceKind::Cli
    }
    fn pull(&mut self, _clock: &dyn Clock) -> Result<Vec<EventEnvelope>, Self::Error> {
        Ok(std::mem::take(&mut self.batch))
    }
}

pub struct FakeSink {
    pub written: RefCell<Vec<EventId>>,
    pub fail_at: Option<usize>,
}
impl EventSink for FakeSink {
    fn append(&self, envelope: &EventEnvelope) -> Result<(), SinkError> {
        let mut written = self.written.borrow_mut();
        if written.contains(&envelope.id) {
            return Ok(());
        }
        if Some(written.len()) == self.fail_at {
            return Err(SinkError::Unavailable("fake failure".to_string()));
        }
        written.push(envelope.id.clone());
        Ok(())
    }
}
