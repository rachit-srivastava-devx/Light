//! Provider port — the only interface to external systems.
//! Credentials are injected by the parent; never serialized in effects or acks.
use crate::effect::{BrokerError, ExternalEffect, ProviderAck, Readback};

/// External provider interface. Implementations must not expose credentials in the ack.
pub trait Provider {
    /// Attempt to dispatch the effect. Called after DISPATCHING state is persisted.
    fn dispatch(&self, e: &ExternalEffect) -> Result<ProviderAck, BrokerError>;
    /// Read back the current remote state for idempotency key `key`.
    fn readback(&self, key: &str) -> Result<Readback, BrokerError>;
}
