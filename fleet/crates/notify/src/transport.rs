//! Transport port: trait, acknowledgement type, and incoming event.
use crate::{Notification, NotifyError};

/// Opaque acknowledgement from a transport provider.
#[derive(Debug, Clone)]
pub struct TransportAck {
    pub provider_ref: Option<String>,
}

/// Port that all transport adapters implement.
pub trait Transport: Send {
    fn send(&self, n: &Notification) -> Result<TransportAck, NotifyError>;
}

/// Incoming event that triggers a notification on state change.
#[derive(Debug, Clone)]
pub struct StateChangeEvent {
    pub task_id: String,
    pub state: String,
    pub payload: String,
    pub user_email: String,
}
