//! Durable outbox: effect record, idempotency guard, enqueue, and emit.
use crate::redact::{derive_idempotency_key, redact_pii};
use crate::transport::StateChangeEvent;
use crate::{DeliveryReceipt, DeliveryStatus, Grant, Notification, NotifyError};

/// A stored outbox record representing durable notification intent.
#[derive(Debug, Clone)]
pub struct OutboxRecord {
    pub effect_id: String,
    pub idempotency_key: String,
}

/// Minimal durable outbox store. Implementations must be single-writer.
pub trait OutboxStore {
    fn insert(&mut self, record: OutboxRecord) -> Result<(), NotifyError>;
    fn contains_key(&self, key: &str) -> bool;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Atomically enqueue a notification (durable intent before any transport send).
/// Returns the `effect_id`. Returns `Conflict` if the idempotency key already exists.
pub fn enqueue(store: &mut impl OutboxStore, n: Notification) -> Result<String, NotifyError> {
    if store.contains_key(&n.idempotency_key) {
        return Err(NotifyError::Conflict);
    }
    let effect_id = format!("eff-{}", n.idempotency_key);
    store.insert(OutboxRecord {
        effect_id: effect_id.clone(),
        idempotency_key: n.idempotency_key,
    })?;
    Ok(effect_id)
}

/// Build and enqueue a `Notification` from a `StateChangeEvent`. Redacts PII first.
pub fn emit_notification(
    store: &mut impl OutboxStore,
    event: &StateChangeEvent,
    transport: &str,
    recipient: &str,
    grant: Grant,
) -> Result<(Notification, DeliveryReceipt), NotifyError> {
    let redacted = redact_pii(&event.payload);
    let key = derive_idempotency_key(&event.task_id, transport, recipient, &redacted);
    let n = Notification {
        task_id: event.task_id.clone(),
        transport: transport.to_string(),
        recipient: recipient.to_string(),
        payload: redacted,
        idempotency_key: key,
        grant,
    };
    let effect_id = enqueue(store, n.clone())?;
    let receipt = DeliveryReceipt {
        effect_id,
        status: DeliveryStatus::Unknown,
        attempts: 0,
        checked: 1,
        total: 1,
    };
    Ok((n, receipt))
}
