//! `DashboardSink`: SSE/WS server, per-run, localhost-only, no daemon. Accepts every event.

mod broadcast;
mod server;

use std::net::SocketAddr;

use broadcast::EventBus;

use crate::event::StreamEvent;
use crate::sink::{Sink, SinkError};

pub struct DashboardSink {
    bus: EventBus,
    addr: Option<SocketAddr>,
}

impl DashboardSink {
    pub fn new() -> Self {
        Self { bus: EventBus::new(256), addr: None }
    }

    /// The bound address, once the server has started (after the first `deliver`).
    pub fn addr(&self) -> Option<SocketAddr> {
        self.addr
    }
}

impl Default for DashboardSink {
    fn default() -> Self {
        Self::new()
    }
}

impl Sink for DashboardSink {
    fn id(&self) -> &'static str {
        "dashboard"
    }

    fn accepts(&self, _event: &StreamEvent) -> bool {
        true
    }

    fn deliver(&mut self, event: &StreamEvent) -> Result<(), SinkError> {
        if self.addr.is_none() {
            self.addr = Some(server::spawn(self.bus.clone()).map_err(|err| SinkError::Permanent {
                sink: "dashboard",
                seq: event.seq(),
                reason: format!("failed to bind dashboard server: {err}"),
            })?);
        }
        self.bus.publish(event.clone());
        Ok(())
    }
}
