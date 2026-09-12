//! Effect types, state machine, store port, and error taxonomy.
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A consumed, parent-authorized capability grant.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Grant {
    pub action: String,
    pub resource: String,
    pub payload_hash: String,
    pub consumed: bool,
    pub expires_at: i64,
}

/// A single external effect — the only path through the broker.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExternalEffect {
    pub effect_id: String,
    pub action: String,
    pub resource: String,
    pub payload_hash: String,
    pub idempotency_key: String,
    pub grant: Grant,
}

/// Durable state machine for an external effect.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffectState {
    Prepared,
    Dispatching,
    Acked,
    NotSeen,
    Failed,
    Unknown,
    Conflict,
    Expired,
}

/// Sanitized acknowledgement from a provider — no credentials.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderAck {
    pub key: String,
    pub provider_ref: String,
}

/// Provider readback result for reconciliation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Readback {
    pub key: String,
    pub state: EffectState,
}

/// Execution receipt — every outcome produces one.
pub struct Receipt {
    pub key: String,
    pub state: EffectState,
    pub checked: usize,
    pub total: usize,
}

/// Durable store port. Implementations must be atomic per key.
pub trait EffectStore {
    fn load(&self, key: &str) -> Option<EffectState>;
    fn save(&mut self, key: &str, state: EffectState) -> Result<(), BrokerError>;
}

/// Typed failure taxonomy (§6).
#[derive(Debug, Error)]
pub enum BrokerError {
    #[error("grant error: {0}")]
    Grant(String),
    #[error("store error: {0}")]
    Store(String),
    #[error("provider error: {0}")]
    Provider(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("reconcile error: {0}")]
    Reconcile(String),
}
