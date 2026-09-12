//! Notification outbox: redaction, durable outbox, transport port, reconciliation.
pub mod outbox;
pub mod reconcile;
pub mod redact;
pub mod transport;

pub use outbox::enqueue;
pub use reconcile::build_delivery_receipt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Grant {
    pub transport: String,
    pub recipient: String,
    pub scope: String,
    pub expiry: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub task_id: String,
    pub transport: String,
    pub recipient: String,
    pub payload: String,
    pub idempotency_key: String,
    pub grant: Grant,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DeliveryStatus { Delivered, Failed, Unknown }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryReceipt {
    pub effect_id: String,
    pub status: DeliveryStatus,
    pub attempts: u64,
    pub checked: u64,
    pub total: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum NotifyError {
    #[error("conflict: duplicate idempotency key")] Conflict,
    #[error("grant check failed")] Grant,
    #[error("redaction error")] Redaction,
    #[error("unknown delivery outcome")] Unknown,
    #[error("zero denominator: total_recipients must be nonzero")] ZeroDenominator,
    #[error("transport error: {0}")] Transport(String),
}
