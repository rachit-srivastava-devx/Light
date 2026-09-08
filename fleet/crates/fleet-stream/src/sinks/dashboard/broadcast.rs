//! The `broadcast::Sender<StreamEvent>` the SSE/WS handlers subscribe to.

use tokio::sync::broadcast;

use crate::event::StreamEvent;

/// Shared internal channel: `DashboardSink::deliver` publishes, `server.rs`'s handlers
/// subscribe. A `send` with zero subscribers is not an error -- nobody watching is not a
/// delivery failure.
#[derive(Clone)]
pub(super) struct EventBus {
    sender: broadcast::Sender<StreamEvent>,
}

impl EventBus {
    pub(super) fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    pub(super) fn publish(&self, event: StreamEvent) {
        let _ = self.sender.send(event);
    }

    pub(super) fn subscribe(&self) -> broadcast::Receiver<StreamEvent> {
        self.sender.subscribe()
    }
}
